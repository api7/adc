//! Real-server replacement for the old mock-based `tests/operator.rs`.
//! Real network calls against a live apisix — see `e2e_apisix.rs`'s module
//! doc for how to bring one up and run this file.
//!
//! What's *not* here, and why:
//! - Create/delete ordering for a service's default upstream (upstream
//!   before service on create, service before upstream on delete) isn't
//!   re-tested here: apisix itself enforces the referential integrity that
//!   ordering exists for — PUTting a service that references a nonexistent
//!   upstream id, or DELETEing an upstream a service still references, both
//!   get rejected with a 400 (confirmed against a real instance). Every
//!   other e2e test that successfully creates/deletes a service with an
//!   inline upstream (`e2e_resource_service_upstream.rs`,
//!   `e2e_sync_and_dump.rs`, ...) is already, necessarily, proof the
//!   ordering is correct — if it weren't, those creates/deletes would fail
//!   outright, not silently pass.
//! - Retry-on-failure (`RetryPolicy`'s own behavior is unit-tested in
//!   `adc-backend-core`; the operator's use of it is a single `.run(...)`
//!   call, not worth re-proving against a live server that would need to be
//!   made to misbehave on purpose to exercise it).
//! - Event grouping (`group_events`) is a pure function with no HTTP
//!   involved at all — see the inline `#[cfg(test)]` unit tests in
//!   `operator.rs` instead.

use std::time::Duration;

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_apisix::tests::Operator;
use adc_backend_core::Method;
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, Event, EventKind, PathSegment, ResourceType, ValueDiff};
use serde_json::{Value, json};

mod common;
use common::{backend, client};

/// A PUT bumps `update_time` unconditionally, even for byte-identical
/// content (confirmed against a real instance) — so comparing it before and
/// after an operation directly answers "was this resource actually
/// written to", without needing to record wire traffic.
///
/// `None` specifically means "the resource doesn't currently exist / has no
/// `update_time`" (a non-success status, or an unparsable body) — a real
/// transport failure panics with context instead of silently becoming
/// `None` too, so a broken docker-compose stack fails loudly here rather
/// than surfacing as a confusing `unwrap()` panic at the call site.
async fn update_time(path: &str) -> Option<i64> {
    let client = client();
    let request = client.request(Method::GET, path).expect("building the update_time request should never fail for a well-formed path");
    let response = client.execute(request).await.expect("transport failure while polling update_time");
    if !response.status().is_success() {
        return None;
    }
    let body: Value = response.json().await.ok()?;
    body["value"]["update_time"].as_i64()
}

/// The resource's own body (the `value` envelope's contents), or `None` if
/// it doesn't currently exist. Same panic-vs-`None` split as `update_time`.
async fn get_value(path: &str) -> Option<Value> {
    let client = client();
    let request = client.request(Method::GET, path).expect("building the get_value request should never fail for a well-formed path");
    let response = client.execute(request).await.expect("transport failure while reading a resource");
    if !response.status().is_success() {
        return None;
    }
    let body: Value = response.json().await.ok()?;
    Some(body["value"].clone())
}

/// PUTs a body straight to the admin API, bypassing the operator (and
/// therefore `transform_service`) entirely — used to seed a service the way
/// an APISIX instance predating adc's upstream/service split would have it:
/// the upstream inlined into the service's own body, with no separate
/// `/upstreams/{id}` resource at all.
async fn put_raw(path: &str, body: Value) {
    let client = client();
    let request = client.request(Method::PUT, path).expect("building the put_raw request should never fail for a well-formed path").json(&body);
    let response = client.execute(request).await.expect("transport failure while seeding legacy state");
    assert!(response.status().is_success(), "seeding legacy state at {path} failed: {}", response.status());
}

fn create(rt: ResourceType, id: &str, new_value: Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, id, id)
}

fn delete(rt: ResourceType, id: &str) -> Event {
    Event::new(rt, EventKind::Delete { old_value: json!({}) }, id, id)
}

fn upstream_diff() -> ValueDiff {
    ValueDiff::Edit { path: vec![PathSegment::Key("upstream".into())], lhs: json!({}), rhs: json!({}) }
}

fn plugins_diff() -> ValueDiff {
    ValueDiff::Edit { path: vec![PathSegment::Key("plugins".into())], lhs: json!({}), rhs: json!({}) }
}

async fn sync_ok(backend: &ApisixBackend, events: Vec<Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        let event = result.event.as_ref().expect("apisix always reports one result per event");
        assert!(result.success, "{:?} {}: {:?}", event.resource_type, event.resource_id, result.error);
    }
}

#[tokio::test]
#[ignore]
async fn update_touching_only_upstream_still_rewrites_the_service_record() {
    let backend = backend();
    let service_id = "e2e-op-svc1";
    sync_ok(
        &backend,
        vec![create(
            ResourceType::Service,
            service_id,
            json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
        )],
    )
    .await;
    let service_path = format!("/apisix/admin/services/{service_id}");
    let before = update_time(&service_path).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;
    let event = Event::new(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
            new_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "2.2.2.2", "port": 80, "weight": 1 }] } }),
            diff: Some(vec![upstream_diff()]),
        },
        service_id,
        service_id,
    );
    sync_ok(&backend, vec![event]).await;

    let after = update_time(&service_path).await.unwrap();
    assert!(after > before, "the service record must still be re-written even when only its upstream changed, to convert a remote inline upstream to the split form");

    sync_ok(&backend, vec![delete(ResourceType::Service, service_id)]).await;
}

/// The actual bug this whole file exists to guard against: a service left
/// over from before adc split upstreams out of services still has its
/// upstream inlined, with no `/upstreams/{id}` resource at all. An update
/// that only touches the upstream's content must still split it out —
/// `update_touching_only_upstream_still_rewrites_the_service_record` above
/// only proves a PUT happens, not that it actually produces the split form.
#[tokio::test]
#[ignore]
async fn update_touching_only_upstream_migrates_a_legacy_inline_upstream_to_the_split_form() {
    let service_id = "e2e-op-svc-legacy1";
    let service_path = format!("/apisix/admin/services/{service_id}");
    let upstream_path = format!("/apisix/admin/upstreams/{service_id}");

    put_raw(
        &service_path,
        json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
    )
    .await;
    assert!(get_value(&upstream_path).await.is_none(), "test setup: no separate upstream resource should exist yet");

    let backend = backend();
    let event = Event::new(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
            new_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "2.2.2.2", "port": 80, "weight": 1 }] } }),
            diff: Some(vec![upstream_diff()]),
        },
        service_id,
        service_id,
    );
    sync_ok(&backend, vec![event]).await;

    let service = get_value(&service_path).await.expect("service should still exist");
    assert_eq!(service["upstream_id"], service_id, "service must now reference the split-out upstream by id");
    assert!(service["upstream"].is_null(), "the upstream must no longer be inlined into the service");
    let upstream = get_value(&upstream_path).await.expect("the legacy inline upstream must have been split into its own resource");
    assert_eq!(upstream["nodes"][0]["host"], "2.2.2.2", "the split-out upstream must carry the updated content");

    sync_ok(&backend, vec![delete(ResourceType::Service, service_id)]).await;
}

/// The mirror case: the diff doesn't mention `upstream` at all (only an
/// unrelated field changed), yet a legacy inlined upstream must still be
/// split out — the diff can't tell "already split" apart from "still
/// inline", so writing the upstream can't be conditioned on the diff
/// mentioning it.
#[tokio::test]
#[ignore]
async fn update_touching_only_another_field_migrates_a_legacy_inline_upstream_to_the_split_form() {
    let service_id = "e2e-op-svc-legacy2";
    let service_path = format!("/apisix/admin/services/{service_id}");
    let upstream_path = format!("/apisix/admin/upstreams/{service_id}");

    put_raw(
        &service_path,
        json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
    )
    .await;
    assert!(get_value(&upstream_path).await.is_none(), "test setup: no separate upstream resource should exist yet");

    let backend = backend();
    let event = Event::new(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
            new_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] }, "plugins": { "key-auth": {} } }),
            diff: Some(vec![plugins_diff()]),
        },
        service_id,
        service_id,
    );
    sync_ok(&backend, vec![event]).await;

    let service = get_value(&service_path).await.expect("service should still exist");
    assert_eq!(service["upstream_id"], service_id, "service must now reference the split-out upstream by id");
    assert!(service["upstream"].is_null(), "the upstream must no longer be inlined into the service");
    let upstream = get_value(&upstream_path).await.expect("the legacy inline upstream must have been split into its own resource, even though the diff never mentioned it");
    assert_eq!(upstream["nodes"][0]["host"], "1.1.1.1", "the split-out upstream must carry the (unchanged) node content");

    sync_ok(&backend, vec![delete(ResourceType::Service, service_id)]).await;
}

#[tokio::test]
#[ignore]
async fn update_touching_both_writes_both_records() {
    let backend = backend();
    let service_id = "e2e-op-svc2";
    sync_ok(
        &backend,
        vec![create(
            ResourceType::Service,
            service_id,
            json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
        )],
    )
    .await;
    let service_path = format!("/apisix/admin/services/{service_id}");
    let upstream_path = format!("/apisix/admin/upstreams/{service_id}");
    let service_before = update_time(&service_path).await.unwrap();
    let upstream_before = update_time(&upstream_path).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;
    let event = Event::new(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
            new_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "2.2.2.2", "port": 80, "weight": 1 }] }, "plugins": { "key-auth": {} } }),
            diff: Some(vec![upstream_diff(), plugins_diff()]),
        },
        service_id,
        service_id,
    );
    sync_ok(&backend, vec![event]).await;

    let service_after = update_time(&service_path).await.unwrap();
    let upstream_after = update_time(&upstream_path).await.unwrap();
    assert!(service_after > service_before, "service record must be rewritten when a non-upstream field changed too");
    assert!(upstream_after > upstream_before, "upstream record must be rewritten when the upstream changed");

    sync_ok(&backend, vec![delete(ResourceType::Service, service_id)]).await;
}

#[tokio::test]
#[ignore]
async fn update_not_touching_upstream_still_rewrites_the_upstream_record() {
    let backend = backend();
    let service_id = "e2e-op-svc3";
    sync_ok(
        &backend,
        vec![create(
            ResourceType::Service,
            service_id,
            json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
        )],
    )
    .await;
    let upstream_path = format!("/apisix/admin/upstreams/{service_id}");
    let before = update_time(&upstream_path).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;
    let event = Event::new(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] } }),
            new_value: json!({ "name": service_id, "upstream": { "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] }, "plugins": { "key-auth": {} } }),
            diff: Some(vec![plugins_diff()]),
        },
        service_id,
        service_id,
    );
    sync_ok(&backend, vec![event]).await;

    let after = update_time(&upstream_path).await.unwrap();
    assert!(after > before, "the upstream record must still be re-written even when the diff doesn't mention it, since the diff can't tell an already-split upstream from a remote inline one");

    sync_ok(&backend, vec![delete(ResourceType::Service, service_id)]).await;
}

#[tokio::test]
#[ignore]
async fn stream_route_below_minimum_version_is_rejected_without_a_request() {
    // A real client pointed at a real (supporting) server, but the
    // `Operator` is deliberately told it's talking to an old one — the
    // version check is entirely client-side, so this doesn't need an
    // actual old apisix to prove it never sends the request.
    let operator = Operator::new(client(), semver::Version::new(3, 5, 0));
    let stream_route_id = "e2e-op-sr1";
    let mut event = create(ResourceType::StreamRoute, stream_route_id, json!({ "name": stream_route_id, "server_port": 34000 }));
    event.parent_id = Some("nonexistent-service".to_string());

    let results = operator.sync(vec![event], BackendSyncOptions::default()).await.unwrap();
    assert!(!results[0].success);
    assert!(matches!(results[0].error, Some(adc_sdk::BackendError::Unsupported(_))), "{:?}", results[0].error);
}

#[tokio::test]
#[ignore]
async fn consumer_credential_below_minimum_version_is_rejected_without_a_request() {
    let operator = Operator::new(client(), semver::Version::new(3, 10, 0));
    let credential_id = "e2e-op-cred1";
    let mut event = create(ResourceType::ConsumerCredential, credential_id, json!({ "name": credential_id, "type": "key-auth", "config": {} }));
    event.parent_id = Some("nonexistent-consumer".to_string());

    let results = operator.sync(vec![event], BackendSyncOptions::default()).await.unwrap();
    assert!(!results[0].success);
    assert!(matches!(results[0].error, Some(adc_sdk::BackendError::Unsupported(_))), "{:?}", results[0].error);
}

#[tokio::test]
#[ignore]
async fn stops_starting_new_groups_after_a_failure_by_default() {
    // The failure here is entirely client-side (a route event with no
    // `parent_id` fails apisix-side path construction before any request
    // is sent), so this doesn't depend on the server misbehaving either.
    // It's a real `operate` failure (not a version-gate rejection), so with
    // the default `exit_on_failure: true` it aborts the whole call as an
    // `Err` rather than a partial result list — matching the TS
    // implementation's `Observable` erroring out instead of completing.
    let backend = backend();
    let bad_route_id = "e2e-op-bad-route";
    let consumer_username = "e2e_op_should_not_run";
    let bad_route = create(ResourceType::Route, bad_route_id, json!({ "name": bad_route_id, "uris": ["/x"] }));
    let consumer = create(ResourceType::Consumer, consumer_username, json!({ "username": consumer_username }));

    let result = backend.sync(vec![bad_route, consumer], BackendSyncOptions::default()).await;
    assert!(result.is_err(), "{result:?}");

    let config = backend.dump().await.unwrap();
    let consumers = config.consumers.unwrap_or_default();
    assert!(consumers.iter().all(|c| c.username != consumer_username), "the consumer from the second group must not have been created");
}
