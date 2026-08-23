//! Applying a differ's `Event`s to a live APISIX instance: `sync`.
//!
//! Two pieces of behavior are worth calling out because they're
//! APISIX-specific rather than generic "call the admin API" plumbing:
//!
//! - A service's default upstream is a *separate* admin-API resource
//!   (`/apisix/admin/upstreams/{id}`, same id as the service), so a single
//!   `SERVICE` event can turn into up to two requests, and their order
//!   matters: creating/updating writes the upstream first (routes/services
//!   referencing it must find it already there), deleting removes the
//!   service first (nothing should reference the upstream by the time it's
//!   removed). An update where only the upstream actually changed skips the
//!   service request entirely.
//! - Requests within one event are applied *sequentially* (a service's
//!   upstream write must complete before its own write starts); events are
//!   grouped by `(resource_type, event_type)` and applied *sequentially
//!   across groups* but *concurrently within a group* (bounded by
//!   `BackendSyncOptions::concurrent`), then retried individually on
//!   failure. Grouping preserves each resource type's relative event order
//!   while still letting e.g. all SSL creates happen before any upstream
//!   creates, matching the differ's own topological ordering.

use adc_backend_core::{HttpClient, Method, RetryPolicy, concurrent_map, concurrent_map_until_err, encode_path_segment};
use adc_sdk::resources::{self as adc};
use adc_sdk::{
    BackendError, BackendSyncOptions, BackendSyncResult, DEFAULT_EXIT_ON_FAILURE, Event, EventType, PathSegment,
    ResourceType, SYNC_EVENT_SPAN_NAME, ValueDiff,
};
use semver::Version;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::transformer;
use crate::typing;
use crate::utils::resource_type_to_api_name;

pub struct Operator {
    client: HttpClient,
    version: Version,
    retry_policy: RetryPolicy,
}

impl Operator {
    pub fn new(client: HttpClient, version: Version) -> Self {
        Self { client, version, retry_policy: RetryPolicy::default() }
    }

    /// Applies `events`. A version-gate rejection (`check_version_support`)
    /// is always a normal `success: false` result, never an abort, since
    /// it's produced before `operate` (the HTTP call) ever runs. An actual
    /// `operate` failure, on the other hand, aborts the whole call as an
    /// `Err` when `exit_on_failure` is set (the default): events already
    /// dispatched within that group still run to completion (there's no
    /// cheap way to cancel an in-flight request), but any event still
    /// queued behind `opts.concurrent`'s limit is dropped and never
    /// dispatched at all, and every result — from this group and any
    /// accumulated from earlier ones — is discarded in favor of the single
    /// `Err`.
    pub async fn sync(&self, events: Vec<Event>, opts: BackendSyncOptions) -> Result<Vec<BackendSyncResult>, BackendError> {
        let exit_on_failure = opts.exit_on_failure.unwrap_or(DEFAULT_EXIT_ON_FAILURE);

        let mut results = Vec::new();
        for group in group_events(events) {
            if exit_on_failure {
                let group_results = concurrent_map_until_err(group, opts.concurrent, |event| self.apply(event)).await.map_err(|(_, error)| error)?;
                results.extend(group_results);
            } else {
                let group_results = concurrent_map(group, opts.concurrent, |event| self.apply(event)).await;
                for outcome in group_results {
                    match outcome {
                        Ok(result) => results.push(result),
                        Err((event, error)) => results.push(BackendSyncResult { success: false, event: Some(event), error: Some(error), server: None }),
                    }
                }
            }
        }
        Ok(results)
    }

    /// `Ok` is always a normal outcome (including a version-gate
    /// rejection); `Err` is specifically an `operate` (HTTP) failure, which
    /// `sync` treats differently depending on `exit_on_failure`.
    ///
    /// Wrapped in a real `SYNC_EVENT_SPAN_NAME` span for the event's whole
    /// lifetime, not a synthetic start/finish pair — `success`/`error` are
    /// recorded just before it closes.
    #[tracing::instrument(
        name = SYNC_EVENT_SPAN_NAME,
        skip_all,
        fields(
            resource_type = %event.resource_type,
            resource_name = %event.resource_name,
            event_type = ?event.event_type(),
            success = tracing::field::Empty,
            error = tracing::field::Empty,
        )
    )]
    // `Event` carries a resource's full JSON body (`EventKind::Update` alone
    // holds two `serde_json::Value`s), so the `Err` side is inherently
    // sizable — boxing it wouldn't help, since `BackendSyncResult` (the `Ok`
    // side) carries an `Option<Event>` of its own and is already comparably
    // large.
    #[allow(clippy::result_large_err)]
    async fn apply(&self, event: Event) -> Result<BackendSyncResult, (Event, BackendError)> {
        let outcome = self.apply_inner(event).await;

        let error_text = match &outcome {
            Ok(result) if !result.success => result.error.as_ref().map(BackendError::to_string),
            Err((_, error)) => Some(error.to_string()),
            _ => None,
        };
        let span = tracing::Span::current();
        match &error_text {
            Some(error) => {
                span.record("success", false);
                span.record("error", error.as_str());
            }
            None => {
                span.record("success", true);
            }
        }
        outcome
    }

    // See the `#[allow(...)]` on `apply` above — same reasoning.
    #[allow(clippy::result_large_err)]
    async fn apply_inner(&self, event: Event) -> Result<BackendSyncResult, (Event, BackendError)> {
        if let Err(error) = self.check_version_support(&event) {
            log::warn!("skipping {:?} {:?} \"{}\": {error}", event.event_type(), event.resource_type, event.resource_name);
            return Ok(BackendSyncResult { success: false, event: Some(event), error: Some(error), server: None });
        }

        match self.operate(&event).await {
            Ok(()) => Ok(BackendSyncResult { success: true, event: Some(event), error: None, server: None }),
            Err(error) => Err((event, error)),
        }
    }

    fn check_version_support(&self, event: &Event) -> Result<(), BackendError> {
        if event.resource_type == ResourceType::StreamRoute && self.version < Version::new(3, 7, 0) {
            return Err(BackendError::Unsupported(
                "stream routes are not supported on apisix versions below 3.7.0".to_string(),
            ));
        }
        if event.resource_type == ResourceType::ConsumerCredential && self.version < Version::new(3, 11, 0) {
            return Err(BackendError::Unsupported(
                "consumer credentials are not supported on apisix versions below 3.11.0".to_string(),
            ));
        }
        Ok(())
    }

    /// Runs one event's requests sequentially, each with a retry policy.
    ///
    /// A `DELETE` on the upstream sub-resource specifically tolerates a
    /// 404: a service's default upstream only exists when the service was
    /// actually created with one (see `build_requests`'s `Create` branch),
    /// so deleting a service that never had one would otherwise fail this
    /// whole event on a request for a resource that was never there to
    /// begin with. Depending on `event.kind.old_value()` accurately
    /// recording that instead would work for the differ's own real Delete
    /// events (which do carry the full prior state) but not for a
    /// hand-built one with an incomplete `old_value` — treating "already
    /// gone" as success here doesn't depend on that being reliable.
    async fn operate(&self, event: &Event) -> Result<(), BackendError> {
        for (method, path, body, kind) in build_requests(event, &self.version)? {
            let tolerate_missing = method == Method::DELETE && kind == RequestKind::Upstream;
            self.retry_policy
                .run(is_retriable, || async {
                    let mut builder = self.client.request(method.clone(), &path)?;
                    if let Some(body) = &body {
                        builder = builder.json(body);
                    }
                    let response = self.client.execute(builder).await?;
                    if tolerate_missing && response.status().as_u16() == 404 {
                        return Ok(());
                    }
                    HttpClient::require_success(response).await.map(|_| ())
                })
                .await?;
        }
        Ok(())
    }
}

/// [`BackendError::is_retriable`] plus APISIX's own retriable case: deleting
/// a resource that's still referenced by another (e.g. an upstream a service
/// still points at) is rejected with a 400 and a message like "can not
/// delete this upstream, service [id] is still using it now". During a
/// sync, the referencing resource is often deleted moments later by a
/// concurrent or later request, so this settles once ordering catches up —
/// unlike other 4xx errors, it isn't final.
fn is_retriable(err: &BackendError) -> bool {
    err.is_retriable()
        || matches!(
            err,
            BackendError::Api { status: 400, message } if message.contains("is still using it now")
        )
}

#[cfg(test)]
mod retry_tests {
    use super::*;

    #[test]
    fn a_dependency_conflict_4xx_is_retriable() {
        let err = BackendError::Api {
            status: 400,
            message: "can not delete this upstream, service [34195131] is still using it now"
                .into(),
        };
        assert!(is_retriable(&err));
    }

    #[test]
    fn a_plain_4xx_api_error_is_not_retriable() {
        let err = BackendError::Api { status: 400, message: "bad config".into() };
        assert!(!is_retriable(&err));
    }

    #[test]
    fn a_5xx_api_error_is_still_retriable() {
        let err = BackendError::Api { status: 502, message: "bad gateway".into() };
        assert!(is_retriable(&err));
    }

    #[test]
    fn a_dependency_conflict_message_on_a_non_400_status_is_not_retriable() {
        let err = BackendError::Api {
            status: 409,
            message: "can not delete this upstream, service [34195131] is still using it now"
                .into(),
        };
        assert!(!is_retriable(&err));
    }
}

/// Buckets events by `(resource_type, event_type)`, preserving each
/// bucket's internal relative order and ordering buckets themselves by
/// first appearance — grouping, not just chunking consecutive runs, so an
/// event can join an earlier bucket even if later events of a different
/// type came between it and its match.
fn group_events(events: Vec<Event>) -> Vec<Vec<Event>> {
    let mut groups: Vec<(ResourceType, EventType, Vec<Event>)> = Vec::new();
    'events: for event in events {
        let key = (event.resource_type, event.event_type());
        for group in &mut groups {
            if (group.0, group.1) == key {
                group.2.push(event);
                continue 'events;
            }
        }
        groups.push((key.0, key.1, vec![event]));
    }
    groups.into_iter().map(|(_, _, events)| events).collect()
}

fn missing_parent(event: &Event) -> BackendError {
    BackendError::Other(format!("{:?} event for resource {:?} is missing a parent_id", event.resource_type, event.resource_id).into())
}

fn deserialize_event_value<T: DeserializeOwned>(value: &Value) -> Result<T, BackendError> {
    serde_json::from_value(value.clone()).map_err(|e| BackendError::Serialization(format!("decoding event payload: {e}")))
}

fn to_request_body<T: Serialize>(value: T) -> Result<Value, BackendError> {
    serde_json::to_value(value).map_err(|e| BackendError::Serialization(format!("encoding request body: {e}")))
}

fn main_path(event: &Event) -> Result<String, BackendError> {
    let resource_id = encode_path_segment(&event.resource_id)?;
    if event.resource_type == ResourceType::ConsumerCredential {
        let parent_id = event.parent_id.as_deref().ok_or_else(|| missing_parent(event))?;
        let parent_id = encode_path_segment(parent_id)?;
        return Ok(format!("/apisix/admin/consumers/{parent_id}/credentials/{resource_id}"));
    }
    let api_name = resource_type_to_api_name(event.resource_type)
        .expect("ConsumerCredential is the only resource type with no api name, and it's handled above");
    Ok(format!("/apisix/admin/{api_name}/{resource_id}"))
}

fn diff_path_is_upstream(diff: &ValueDiff) -> bool {
    let path = match diff {
        ValueDiff::New { path, .. } | ValueDiff::Deleted { path, .. } | ValueDiff::Edit { path, .. } | ValueDiff::Array { path, .. } => path,
    };
    matches!(path.first(), Some(PathSegment::Key(key)) if key == "upstream")
}

/// Which of a `SERVICE` event's (up to two) request paths a given
/// (method, path, body) triple is for — replaces sniffing the path string
/// for `"/upstreams/"` with an explicit tag threaded alongside it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Main,
    Upstream,
}

/// One request `build_requests` produced: method, path, body (`None` for a
/// `DELETE`), and which of a `SERVICE` event's (up to two) requests it is.
type BuiltRequest = (Method, String, Option<Value>, RequestKind);

/// Builds the ordered requests for one event. A non-`SERVICE` event always
/// produces exactly one; a `SERVICE` event produces one or two (see the
/// module doc comment). The `RequestKind` on each is carried through to
/// [`Operator::operate`], which needs it to know whether a `DELETE` is for
/// the upstream sub-resource specifically (see its doc comment for why
/// that one tolerates a 404).
/// Whether a `SERVICE` event's new value still has an `upstream` field —
/// `transform_service` returns `None` when it doesn't, which the upstream
/// sub-request must turn into a DELETE (nothing to PUT) rather than an error.
fn service_has_upstream(event: &Event) -> bool {
    event.kind.new_value().and_then(|v| v.get("upstream")).is_some_and(|v| !v.is_null())
}

fn build_requests(event: &Event, version: &Version) -> Result<Vec<BuiltRequest>, BackendError> {
    let is_delete = event.event_type() == EventType::Delete;
    let main_method = if is_delete { Method::DELETE } else { Method::PUT };
    let mut paths = vec![(main_path(event)?, RequestKind::Main, main_method)];

    if event.resource_type == ResourceType::Service {
        let upstream_path = format!("/apisix/admin/upstreams/{}", encode_path_segment(&event.resource_id)?);
        match event.event_type() {
            EventType::Delete => paths.push((upstream_path, RequestKind::Upstream, Method::DELETE)),
            EventType::Create => {
                if service_has_upstream(event) {
                    paths.insert(0, (upstream_path, RequestKind::Upstream, Method::PUT));
                }
            }
            EventType::Update => {
                let diff = event.kind.diff().filter(|d| !d.is_empty()).ok_or_else(|| {
                    BackendError::Other(format!("service {:?} update event is missing diff info", event.resource_id).into())
                })?;
                let touches_non_upstream = diff.iter().any(|d| !diff_path_is_upstream(d));
                let touches_upstream = diff.iter().any(diff_path_is_upstream);
                if !touches_non_upstream {
                    paths.pop();
                }
                if touches_upstream {
                    // Still has an upstream: PUT the new body. Lost it
                    // entirely: nothing to PUT, so DELETE instead — `operate`
                    // already tolerates a 404 here for the "never had one" case.
                    let method = if service_has_upstream(event) { Method::PUT } else { Method::DELETE };
                    paths.insert(0, (upstream_path, RequestKind::Upstream, method));
                }
            }
        }
    }

    paths
        .into_iter()
        .map(|(path, kind, method)| {
            let body = if method == Method::DELETE { None } else { Some(request_body(event, kind, version)?) };
            Ok((method, path, body, kind))
        })
        .collect()
}

/// Builds the JSON body for one request. For a `SERVICE` event this is
/// called once per request path, and `kind` says which of the (up to two)
/// it's building for.
fn request_body(event: &Event, kind: RequestKind, version: &Version) -> Result<Value, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| BackendError::Other("create/update event is missing new_value".into()))?;

    match event.resource_type {
        ResourceType::Consumer => to_request_body(typing::Consumer::from(deserialize_event_value::<adc::Consumer>(new_value)?)),
        ResourceType::ConsumerGroup => {
            let group: adc::ConsumerGroup = deserialize_event_value(new_value)?;
            let (mut wire, _consumers) = transformer::transform_consumer_group(group);
            // `transform_consumer_group` derives its id from the group's
            // name, but `main_path` builds the request URL from
            // `event.resource_id` — APISIX rejects a PUT whose body id
            // doesn't match the URL, so they must match exactly, including
            // when the differ assigned an explicit id independent of the
            // name (`ConsumerGroup` supports a user-specified `id`).
            wire.id = event.resource_id.clone();
            to_request_body(wire)
        }
        ResourceType::ConsumerCredential => {
            let mut credential: adc::ConsumerCredential = deserialize_event_value(new_value)?;
            credential.id = Some(event.resource_id.clone());
            to_request_body(typing::ConsumerCredential::from(credential))
        }
        ResourceType::GlobalRule => Ok(serde_json::json!({ "plugins": { event.resource_id.clone(): new_value.clone() } })),
        ResourceType::PluginMetadata => Ok(new_value.clone()),
        ResourceType::Route => {
            // The differ's event carries the authoritative id
            // (`event.resource_id`); `new_value` itself was never required
            // to have one, so it must be stamped on before transforming —
            // APISIX's admin API rejects a PUT whose body id doesn't match
            // the URL, including when the body's id is empty.
            let mut route: adc::Route = deserialize_event_value(new_value)?;
            route.id = Some(event.resource_id.clone());
            let parent_id = event.parent_id.clone().ok_or_else(|| missing_parent(event))?;
            to_request_body(transformer::transform_route(route, parent_id))
        }
        ResourceType::Service => {
            let mut service: adc::Service = deserialize_event_value(new_value)?;
            service.id = Some(event.resource_id.clone());
            let (wire_service, wire_upstream) = transformer::transform_service(service);
            match kind {
                RequestKind::Upstream => {
                    let upstream = wire_upstream.ok_or_else(|| {
                        BackendError::Other(format!("service {:?} has no default upstream to write", event.resource_id).into())
                    })?;
                    to_request_body(upstream)
                }
                RequestKind::Main => to_request_body(wire_service),
            }
        }
        ResourceType::Ssl => {
            let mut ssl: adc::SSL = deserialize_event_value(new_value)?;
            ssl.id = Some(event.resource_id.clone());
            to_request_body(typing::Ssl::from(ssl))
        }
        ResourceType::StreamRoute => {
            let route: adc::StreamRoute = deserialize_event_value(new_value)?;
            let parent_id = event.parent_id.clone().ok_or_else(|| missing_parent(event))?;
            let inject_name = *version >= Version::new(3, 8, 0);
            to_request_body(transformer::transform_stream_route(route, parent_id, inject_name))
        }
        ResourceType::Upstream => {
            let upstream: adc::Upstream = deserialize_event_value(new_value)?;
            let mut wire = typing::Upstream::from(upstream);
            if let Some(parent_id) = &event.parent_id {
                let mut labels = wire.labels.unwrap_or_default();
                labels.insert(typing::ADC_UPSTREAM_SERVICE_ID_LABEL.to_string(), adc::LabelValue::Single(parent_id.clone()));
                wire.labels = Some(labels);
            }
            to_request_body(wire)
        }
        ResourceType::InternalStreamService => {
            Err(BackendError::Unsupported(format!("{:?} is not directly syncable by the apisix backend", event.resource_type)))
        }
    }
}

#[cfg(test)]
mod tests {
    use adc_sdk::EventKind;
    use serde_json::json;

    use super::*;

    fn event(rt: ResourceType, kind: EventKind, id: &str) -> Event {
        Event::new(rt, kind, id, id)
    }

    fn create(rt: ResourceType, id: &str) -> Event {
        event(rt, EventKind::Create { new_value: json!({}) }, id)
    }

    #[test]
    fn groups_by_resource_and_event_type_preserving_first_seen_order() {
        let route1 = create(ResourceType::Route, "r1");
        let consumer = create(ResourceType::Consumer, "c1");
        let route2 = create(ResourceType::Route, "r2");
        let ssl_delete = event(ResourceType::Ssl, EventKind::Delete { old_value: json!({}) }, "s1");

        let groups = group_events(vec![route1, consumer, route2, ssl_delete]);

        // Buckets ordered by first appearance: (Route, Create) seen first,
        // then (Consumer, Create), then (Ssl, Delete) — even though the
        // second Route event arrives later in the input, it joins the
        // first bucket rather than starting a new one.
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[0][0].resource_id, "r1");
        assert_eq!(groups[0][1].resource_id, "r2");
        assert_eq!(groups[1].len(), 1);
        assert_eq!(groups[1][0].resource_type, ResourceType::Consumer);
        assert_eq!(groups[2].len(), 1);
        assert_eq!(groups[2][0].event_type(), EventType::Delete);
    }

    #[test]
    fn same_resource_type_but_different_event_type_gets_its_own_group() {
        let create_route = create(ResourceType::Route, "r1");
        let delete_route = event(ResourceType::Route, EventKind::Delete { old_value: json!({}) }, "r2");

        let groups = group_events(vec![create_route, delete_route]);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0][0].event_type(), EventType::Create);
        assert_eq!(groups[1][0].event_type(), EventType::Delete);
    }

    #[test]
    fn empty_input_produces_no_groups() {
        assert!(group_events(vec![]).is_empty());
    }

    #[test]
    fn consumer_group_request_body_id_matches_the_event_resource_id_not_the_name() {
        // The differ can assign a ConsumerGroup an explicit, stable id
        // independent of its name; `transform_consumer_group` derives its
        // own id from the name alone, so the request body must be
        // overridden back to the event's id or it'll mismatch the URL
        // path (built from `event.resource_id`) and APISIX will reject it.
        let group_event = Event::new(
            ResourceType::ConsumerGroup,
            EventKind::Create { new_value: json!({ "name": "renamed-group" }) },
            "stable-id",
            "renamed-group",
        );
        let requests = build_requests(&group_event, &Version::new(3, 17, 0)).unwrap();
        assert_eq!(requests.len(), 1);
        let (_, path, body, _) = &requests[0];
        assert!(path.ends_with("/stable-id"), "{path}");
        assert_eq!(body.as_ref().unwrap()["id"], "stable-id");
    }

    #[test]
    fn service_create_without_an_upstream_produces_only_the_main_request() {
        let service_event = event(
            ResourceType::Service,
            EventKind::Create { new_value: json!({ "name": "svc-no-upstream" }) },
            "svc-no-upstream",
        );
        let requests = build_requests(&service_event, &Version::new(3, 17, 0)).unwrap();
        assert_eq!(requests.len(), 1, "no upstream request should be generated for a service with no upstream");
        assert!(requests[0].1.ends_with("/services/svc-no-upstream"), "{}", requests[0].1);
    }

    #[test]
    fn service_update_removing_its_upstream_deletes_the_upstream_resource_instead_of_erroring() {
        let old_value = json!({ "name": "svc1", "upstream": { "nodes": [] } });
        let new_value = json!({ "name": "svc1" });
        let diff = vec![ValueDiff::Deleted { path: vec![PathSegment::Key("upstream".to_string())], lhs: old_value["upstream"].clone() }];
        let service_event =
            Event::new(ResourceType::Service, EventKind::Update { old_value, new_value, diff: Some(diff) }, "svc1", "svc1");

        let requests = build_requests(&service_event, &Version::new(3, 17, 0)).unwrap();
        // Only the upstream field changed, so the service body itself isn't
        // re-sent (same rule as any other upstream-only update) — just the
        // DELETE for the upstream that's no longer there.
        assert_eq!(requests.len(), 1);
        let (method, path, body, kind) = &requests[0];
        assert!(matches!(kind, RequestKind::Upstream));
        assert_eq!(*method, Method::DELETE);
        assert!(body.is_none());
        assert!(path.ends_with("/upstreams/svc1"), "{path}");
    }

    #[test]
    fn service_update_with_no_diff_is_rejected_instead_of_silently_sending_nothing() {
        let service_event = Event::new(
            ResourceType::Service,
            EventKind::Update { old_value: json!({}), new_value: json!({}), diff: None },
            "svc1",
            "svc1",
        );
        assert!(build_requests(&service_event, &Version::new(3, 17, 0)).is_err());
    }

    #[test]
    fn resource_id_containing_a_path_separator_is_percent_encoded_not_split() {
        let route_event = {
            let mut e = event(
                ResourceType::Route,
                EventKind::Create { new_value: json!({ "name": "r1", "uris": ["/x"] }) },
                "a/../b",
            );
            e.parent_id = Some("svc1".to_string());
            e
        };
        let requests = build_requests(&route_event, &Version::new(3, 17, 0)).unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].1.starts_with("/apisix/admin/routes/"), "{}", requests[0].1);
        assert!(!requests[0].1.contains("/../"), "{}", requests[0].1);
    }
}
