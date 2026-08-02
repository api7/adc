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

use adc_backend_core::{HttpClient, Method, RetryPolicy, concurrent_map, concurrent_map_until_err};
use adc_sdk::resources::{self as adc};
use adc_sdk::{BackendError, BackendSyncOptions, BackendSyncResult, Event, EventType, PathSegment, ResourceType, ValueDiff};
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

    /// Applies `events`. Matches the TS implementation: a version-gate
    /// rejection (`check_version_support`) is always a normal
    /// `success: false` result, never an abort, since it's produced before
    /// the point in the pipeline TS's `catchError`/`exitOnFailure` logic
    /// applies. An actual `operate` (HTTP) failure, on the other hand,
    /// aborts the whole call as an `Err` when `exit_on_failure` is set (the
    /// default) — mirroring RxJS `mergeMap` unsubscribing on error:
    /// events already dispatched within that group still run to completion
    /// (there's no cheap way to cancel an in-flight request), but any event
    /// still queued behind `opts.concurrent`'s limit is dropped and never
    /// dispatched at all, and every result — from this group and any
    /// accumulated from earlier ones — is discarded in favor of the single
    /// `Err`.
    pub async fn sync(&self, events: Vec<Event>, opts: BackendSyncOptions) -> Result<Vec<BackendSyncResult>, BackendError> {
        let exit_on_failure = opts.exit_on_failure.unwrap_or(true);

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
                        Err((event, error)) => results.push(BackendSyncResult { success: false, event, error: Some(error), server: None }),
                    }
                }
            }
        }
        Ok(results)
    }

    /// `Ok` is always a normal outcome (including a version-gate
    /// rejection); `Err` is specifically an `operate` (HTTP) failure, which
    /// `sync` treats differently depending on `exit_on_failure`.
    async fn apply(&self, event: Event) -> Result<BackendSyncResult, (Event, BackendError)> {
        if let Err(error) = self.check_version_support(&event) {
            return Ok(BackendSyncResult { success: false, event, error: Some(error), server: None });
        }

        match self.operate(&event).await {
            Ok(()) => Ok(BackendSyncResult { success: true, event, error: None, server: None }),
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
    async fn operate(&self, event: &Event) -> Result<(), BackendError> {
        for (method, path, body) in build_requests(event, &self.version)? {
            self.retry_policy
                .run(|| async {
                    let mut builder = self.client.request(method.clone(), &path)?;
                    if let Some(body) = &body {
                        builder = builder.json(body);
                    }
                    self.client.send(builder).await
                })
                .await?;
        }
        Ok(())
    }
}

/// Buckets events by `(resource_type, event_type)`, preserving each
/// bucket's internal relative order and ordering buckets themselves by
/// first appearance — the same "group, don't just chunk consecutive runs"
/// semantics as the TS operator's `reduce`-into-buckets step.
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
    if event.resource_type == ResourceType::ConsumerCredential {
        let parent_id = event.parent_id.as_deref().ok_or_else(|| missing_parent(event))?;
        return Ok(format!("/apisix/admin/consumers/{parent_id}/credentials/{}", event.resource_id));
    }
    Ok(format!("/apisix/admin/{}/{}", resource_type_to_api_name(event.resource_type), event.resource_id))
}

fn diff_path_is_upstream(diff: &ValueDiff) -> bool {
    let path = match diff {
        ValueDiff::New { path, .. } | ValueDiff::Deleted { path, .. } | ValueDiff::Edit { path, .. } | ValueDiff::Array { path, .. } => path,
    };
    matches!(path.first(), Some(PathSegment::Key(key)) if key == "upstream")
}

/// Builds the ordered (method, path, body) triples for one event. A
/// non-`SERVICE` event always produces exactly one request; a `SERVICE`
/// event produces one or two (see the module doc comment).
fn build_requests(event: &Event, version: &Version) -> Result<Vec<(Method, String, Option<Value>)>, BackendError> {
    let is_delete = event.event_type() == EventType::Delete;
    let mut paths = vec![main_path(event)?];

    if event.resource_type == ResourceType::Service {
        let upstream_path = format!("/apisix/admin/upstreams/{}", event.resource_id);
        match event.event_type() {
            EventType::Delete => paths.push(upstream_path),
            EventType::Create => paths.insert(0, upstream_path),
            EventType::Update => {
                let diff = event.kind.diff().unwrap_or(&[]);
                let touches_non_upstream = diff.iter().any(|d| !diff_path_is_upstream(d));
                let touches_upstream = diff.iter().any(diff_path_is_upstream);
                if !touches_non_upstream {
                    paths.pop();
                }
                if touches_upstream {
                    paths.insert(0, upstream_path);
                }
            }
            EventType::OnlySubEvents => {}
        }
    }

    paths
        .into_iter()
        .map(|path| {
            let method = if is_delete { Method::DELETE } else { Method::PUT };
            let body = if is_delete { None } else { Some(request_body(event, &path, version)?) };
            Ok((method, path, body))
        })
        .collect()
}

/// Builds the JSON body for one request. For a `SERVICE` event this is
/// called once per request path, and returns a different body depending on
/// which of the (up to two) paths it's building for.
fn request_body(event: &Event, path: &str, version: &Version) -> Result<Value, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| BackendError::Other("create/update event is missing new_value".into()))?;

    match event.resource_type {
        ResourceType::Consumer => to_request_body(typing::Consumer::from(deserialize_event_value::<adc::Consumer>(new_value)?)),
        ResourceType::ConsumerGroup => {
            // `transform_consumer_group` derives its own id from the group's
            // name and ignores whatever's here, but set it anyway for
            // parity with the other resource types that do rely on it.
            let mut group: adc::ConsumerGroup = deserialize_event_value(new_value)?;
            group.id = Some(event.resource_id.clone());
            let (wire, _consumers) = transformer::transform_consumer_group(group);
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
            if path.contains("/upstreams/") {
                let upstream = wire_upstream.ok_or_else(|| {
                    BackendError::Other(format!("service {:?} has no default upstream to write", event.resource_id).into())
                })?;
                to_request_body(upstream)
            } else {
                to_request_body(wire_service)
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
        ResourceType::PluginConfig | ResourceType::InternalStreamService => {
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
}
