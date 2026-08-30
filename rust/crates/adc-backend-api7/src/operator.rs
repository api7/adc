//! Applying a differ's `Event`s to a live API7 Enterprise gateway group:
//! `sync`.
//!
//! Unlike APISIX, a service's default upstream is embedded directly in its
//! own body (see `typing::Service`'s `From` impl's doc comment) — a
//! `SERVICE` event is always exactly one request, not up to two. Named
//! (non-default) upstreams for canary release still address their own
//! nested collection (`/apisix/admin/services/{parent}/upstreams/{id}`).
//!
//! No retry wrapping here — unlike `adc_backend_apisix::Operator`, a
//! failed request is never retried.
//!
//! Before applying, events go through preprocessing: a route/stream_route/
//! upstream/credential `DELETE` whose parent is *also* being deleted in
//! this same batch is dropped (the parent's delete cascades it already),
//! then events are grouped by `(resource_type, event_type)`, preserving
//! relative order within and across groups — events within one group run
//! concurrently (bounded by `BackendSyncOptions::concurrent`), groups run
//! sequentially.

use std::collections::HashSet;

use adc_backend_core::{
    HttpClient, Method, RequestBuilder, concurrent_map, concurrent_map_until_err, deserialize_event_value,
    encode_path_segment, missing_parent, to_request_body,
};
use adc_sdk::resources::{self as adc};
use adc_sdk::{
    BackendError, BackendSyncOptions, BackendSyncResult, DEFAULT_EXIT_ON_FAILURE, Event, EventType, ResourceType,
    SYNC_EVENT_SPAN_NAME,
};
use serde_json::Value;

use crate::transformer;
use crate::typing;
use crate::utils::resource_type_to_api_name;

pub struct Operator {
    client: HttpClient,
    gateway_group_id: Option<String>,
}

impl Operator {
    pub fn new(client: HttpClient, gateway_group_id: Option<String>) -> Self {
        Self {
            client,
            gateway_group_id,
        }
    }

    /// An actual `operate` (HTTP) failure aborts the whole call as an
    /// `Err` when `exit_on_failure` is set (the default), discarding
    /// results accumulated so far. Events already dispatched within that
    /// group still run to completion (no cheap way to cancel an in-flight
    /// request), but anything still queued behind the group's concurrency
    /// limit is dropped and never dispatched.
    pub async fn sync(
        &self,
        events: Vec<Event>,
        opts: BackendSyncOptions,
    ) -> Result<Vec<BackendSyncResult>, BackendError> {
        let exit_on_failure = opts.exit_on_failure.unwrap_or(DEFAULT_EXIT_ON_FAILURE);

        let mut results = Vec::new();
        for group in group_events(preprocess_events(events)) {
            let concurrent = group_concurrency(&group, opts.concurrent);
            if exit_on_failure {
                let group_results =
                    concurrent_map_until_err(group, concurrent, |event| self.apply(event))
                        .await
                        .map_err(|(_, error)| error)?;
                results.extend(group_results);
            } else {
                let group_results =
                    concurrent_map(group, concurrent, |event| self.apply(event)).await;
                for outcome in group_results {
                    match outcome {
                        Ok(result) => results.push(result),
                        Err((event, error)) => results.push(BackendSyncResult {
                            success: false,
                            event: Some(event),
                            error: Some(error),
                            server: None,
                            confirmed: None,
                        }),
                    }
                }
            }
        }
        Ok(results)
    }

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
        let outcome = self.operate(&event).await;

        let span = tracing::Span::current();
        match &outcome {
            Ok(()) => span.record("success", true),
            Err(error) => {
                span.record("success", false);
                span.record("error", error.to_string())
            }
        };

        match outcome {
            Ok(()) => Ok(BackendSyncResult {
                success: true,
                event: Some(event),
                error: None,
                server: None,
                confirmed: None,
            }),
            Err(error) => Err((event, error)),
        }
    }

    async fn operate(&self, event: &Event) -> Result<(), BackendError> {
        let path = build_path(event)?;
        let is_delete = event.event_type() == EventType::Delete;

        let mut builder = self.request(
            if is_delete {
                Method::DELETE
            } else {
                Method::PUT
            },
            &path,
        )?;
        if !is_delete {
            builder = builder.json(&request_body(event)?);
        }
        self.client.send(builder).await.map(|_| ())
    }

    fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, BackendError> {
        let mut builder = self.client.request(method, path)?;
        if let Some(id) = &self.gateway_group_id {
            builder = builder.query(&[("gateway_group_id", id)]);
        }
        Ok(builder)
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

/// The concurrency to run one group's events at. Normally just passes
/// `requested` (`opts.concurrent`) straight through, except for
/// `GlobalRule`: some dashboard versions store a gateway group's entire
/// global-rules collection as one shared etcd document, read-modify-written
/// on every PUT — two concurrent global-rule writes race on that document's
/// revision and one of them gets rejected outright. Capping this one
/// resource type to 1 makes syncing multiple global rules in the same
/// batch reliable regardless of dashboard version, at the cost of losing
/// any parallelism between them (a batch is typically small enough that
/// this doesn't matter in practice).
fn group_concurrency(group: &[Event], requested: Option<usize>) -> Option<usize> {
    if group.first().is_some_and(|event| event.resource_type == ResourceType::GlobalRule) {
        Some(1)
    } else {
        requested
    }
}

/// Drops a route/stream_route/upstream `DELETE` whose parent service is
/// also being deleted in this same batch, and a credential `DELETE` whose
/// parent consumer is — the parent's own delete cascades it, so applying
/// it separately would either be redundant or race the parent's delete.
fn preprocess_events(events: Vec<Event>) -> Vec<Event> {
    let deleted_service_ids: HashSet<String> = events
        .iter()
        .filter(|e| e.resource_type == ResourceType::Service && e.event_type() == EventType::Delete)
        .map(|e| e.resource_id.clone())
        .collect();
    let deleted_consumer_ids: HashSet<String> = events
        .iter()
        .filter(|e| {
            e.resource_type == ResourceType::Consumer && e.event_type() == EventType::Delete
        })
        .map(|e| e.resource_id.clone())
        .collect();

    events
        .into_iter()
        .filter(|event| {
            let is_delete = event.event_type() == EventType::Delete;
            let cascaded_by_service = is_delete
                && matches!(
                    event.resource_type,
                    ResourceType::Route | ResourceType::StreamRoute | ResourceType::Upstream
                )
                && event
                    .parent_id
                    .as_deref()
                    .is_some_and(|id| deleted_service_ids.contains(id));
            let cascaded_by_consumer = is_delete
                && event.resource_type == ResourceType::ConsumerCredential
                && event
                    .parent_id
                    .as_deref()
                    .is_some_and(|id| deleted_consumer_ids.contains(id));
            !cascaded_by_service && !cascaded_by_consumer
        })
        .collect()
}

fn build_path(event: &Event) -> Result<String, BackendError> {
    let resource_id = encode_path_segment(&event.resource_id)?;
    match event.resource_type {
        ResourceType::ConsumerCredential => {
            let parent_id = encode_path_segment(
                event
                    .parent_id
                    .as_deref()
                    .ok_or_else(|| missing_parent(event))?,
            )?;
            Ok(format!(
                "/apisix/admin/consumers/{parent_id}/credentials/{resource_id}"
            ))
        }
        ResourceType::Upstream => {
            let parent_id = encode_path_segment(
                event
                    .parent_id
                    .as_deref()
                    .ok_or_else(|| missing_parent(event))?,
            )?;
            Ok(format!(
                "/apisix/admin/services/{parent_id}/upstreams/{resource_id}"
            ))
        }
        _ => {
            let api_name = resource_type_to_api_name(event.resource_type).ok_or_else(|| {
                BackendError::Unsupported(format!(
                    "{:?} has no top-level admin API collection",
                    event.resource_type
                ))
            })?;
            Ok(format!("/apisix/admin/{api_name}/{resource_id}"))
        }
    }
}

fn request_body(event: &Event) -> Result<Value, BackendError> {
    let new_value = event
        .kind
        .new_value()
        .ok_or_else(|| BackendError::Other("create/update event is missing new_value".into()))?;

    match event.resource_type {
        ResourceType::Consumer => {
            to_request_body(typing::Consumer::from(deserialize_event_value::<
                adc::Consumer,
            >(new_value)?))
        }
        ResourceType::GlobalRule => {
            Ok(serde_json::json!({ "plugins": { event.resource_id.clone(): new_value.clone() } }))
        }
        ResourceType::PluginMetadata => Ok(new_value.clone()),
        ResourceType::Service => {
            let mut service: adc::Service = deserialize_event_value(new_value)?;
            service.id = Some(event.resource_id.clone());
            to_request_body(typing::Service::from(service))
        }
        ResourceType::Route => {
            let mut route: adc::Route = deserialize_event_value(new_value)?;
            route.id = Some(event.resource_id.clone());
            let parent_id = event
                .parent_id
                .clone()
                .ok_or_else(|| missing_parent(event))?;
            to_request_body(transformer::transform_route(route, parent_id))
        }
        ResourceType::StreamRoute => {
            let mut route: adc::StreamRoute = deserialize_event_value(new_value)?;
            route.id = Some(event.resource_id.clone());
            let parent_id = event
                .parent_id
                .clone()
                .ok_or_else(|| missing_parent(event))?;
            to_request_body(transformer::transform_stream_route(route, parent_id))
        }
        ResourceType::Ssl => {
            let mut ssl: adc::SSL = deserialize_event_value(new_value)?;
            ssl.id = Some(event.resource_id.clone());
            to_request_body(typing::Ssl::try_from(ssl).map_err(BackendError::Serialization)?)
        }
        ResourceType::ConsumerCredential => {
            let mut credential: adc::ConsumerCredential = deserialize_event_value(new_value)?;
            credential.id = Some(event.resource_id.clone());
            to_request_body(typing::ConsumerCredential::from(credential))
        }
        ResourceType::Upstream => {
            let upstream: adc::Upstream = deserialize_event_value(new_value)?;
            to_request_body(typing::Upstream::from(upstream))
        }
        ResourceType::ConsumerGroup
        | ResourceType::InternalStreamService => Err(BackendError::Unsupported(format!(
            "{:?} is not directly syncable by the api7 backend",
            event.resource_type
        ))),
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
        event(
            rt,
            EventKind::Create {
                new_value: json!({}),
            },
            id,
        )
    }

    fn delete(rt: ResourceType, id: &str) -> Event {
        event(
            rt,
            EventKind::Delete {
                old_value: json!({}),
            },
            id,
        )
    }

    #[test]
    fn groups_by_resource_and_event_type_preserving_first_seen_order() {
        let route1 = create(ResourceType::Route, "r1");
        let consumer = create(ResourceType::Consumer, "c1");
        let route2 = create(ResourceType::Route, "r2");
        let ssl_delete = delete(ResourceType::Ssl, "s1");

        let groups = group_events(vec![route1, consumer, route2, ssl_delete]);

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
    fn global_rule_groups_are_forced_to_a_concurrency_of_one() {
        let group = vec![create(ResourceType::GlobalRule, "g1"), create(ResourceType::GlobalRule, "g2")];
        assert_eq!(group_concurrency(&group, None), Some(1));
        assert_eq!(group_concurrency(&group, Some(10)), Some(1));
    }

    #[test]
    fn other_resource_types_pass_the_requested_concurrency_through_unchanged() {
        let group = vec![create(ResourceType::Route, "r1"), create(ResourceType::Route, "r2")];
        assert_eq!(group_concurrency(&group, None), None);
        assert_eq!(group_concurrency(&group, Some(10)), Some(10));
    }

    #[test]
    fn an_empty_group_passes_the_requested_concurrency_through_unchanged() {
        assert_eq!(group_concurrency(&[], Some(5)), Some(5));
    }

    #[test]
    fn a_route_delete_whose_parent_service_is_also_deleted_is_dropped() {
        let mut route_delete = delete(ResourceType::Route, "r1");
        route_delete.parent_id = Some("svc1".to_string());
        let service_delete = delete(ResourceType::Service, "svc1");

        let remaining = preprocess_events(vec![route_delete, service_delete.clone()]);

        assert_eq!(remaining, vec![service_delete]);
    }

    #[test]
    fn a_credential_delete_whose_parent_consumer_is_also_deleted_is_dropped() {
        let mut credential_delete = delete(ResourceType::ConsumerCredential, "cred1");
        credential_delete.parent_id = Some("user1".to_string());
        let consumer_delete = delete(ResourceType::Consumer, "user1");

        let remaining = preprocess_events(vec![credential_delete, consumer_delete.clone()]);

        assert_eq!(remaining, vec![consumer_delete]);
    }

    #[test]
    fn a_route_delete_whose_parent_service_is_not_deleted_is_kept() {
        let mut route_delete = delete(ResourceType::Route, "r1");
        route_delete.parent_id = Some("svc1".to_string());

        let remaining = preprocess_events(vec![route_delete.clone()]);

        assert_eq!(remaining, vec![route_delete]);
    }

    #[test]
    fn a_non_delete_event_for_a_deleted_services_route_is_kept() {
        // Only a DELETE cascades; e.g. an UPDATE for a route belonging to
        // a service that's simultaneously being deleted is unusual but not
        // this preprocessing step's concern.
        let mut route_update = event(
            ResourceType::Route,
            EventKind::Update {
                old_value: json!({}),
                new_value: json!({}),
                diff: None,
            },
            "r1",
        );
        route_update.parent_id = Some("svc1".to_string());
        let service_delete = delete(ResourceType::Service, "svc1");

        let remaining = preprocess_events(vec![route_update.clone(), service_delete.clone()]);

        assert_eq!(remaining, vec![route_update, service_delete]);
    }

    #[test]
    fn resource_id_containing_a_path_separator_is_percent_encoded_not_split() {
        let mut route_event = create(ResourceType::Route, "a/../b");
        route_event.parent_id = Some("svc1".to_string());

        let path = build_path(&route_event).unwrap();

        assert!(path.starts_with("/apisix/admin/routes/"), "{path}");
        assert!(!path.contains("/../"), "{path}");
    }

    #[test]
    fn a_consumer_credential_path_nests_under_its_parent_consumer() {
        let mut credential_event = create(ResourceType::ConsumerCredential, "cred1");
        credential_event.parent_id = Some("user1".to_string());

        let path = build_path(&credential_event).unwrap();

        assert_eq!(path, "/apisix/admin/consumers/user1/credentials/cred1");
    }

    #[test]
    fn a_named_upstream_path_nests_under_its_parent_service() {
        let mut upstream_event = create(ResourceType::Upstream, "up1");
        upstream_event.parent_id = Some("svc1".to_string());

        let path = build_path(&upstream_event).unwrap();

        assert_eq!(path, "/apisix/admin/services/svc1/upstreams/up1");
    }
}
