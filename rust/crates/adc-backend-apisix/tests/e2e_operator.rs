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
use adc_backend_core::{HttpClient, HttpClientConfig, Method, TlsConfig};
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, Event, EventKind, PathSegment, ResourceType, ValueDiff};
use serde_json::{Value, json};

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn client() -> HttpClient {
    HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap()
}

fn backend() -> ApisixBackend {
    ApisixBackend::new(client())
}

/// A PUT bumps `update_time` unconditionally, even for byte-identical
/// content (confirmed against a real instance) — so comparing it before and
/// after an operation directly answers "was this resource actually
/// written to", without needing to record wire traffic.
async fn update_time(path: &str) -> Option<i64> {
    let request = client().request(Method::GET, path).ok()?;
    let response = client().execute(request).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body: Value = response.json().await.ok()?;
    body["value"]["update_time"].as_i64()
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
        assert!(result.success, "{:?} {}: {:?}", result.event.resource_type, result.event.resource_id, result.error);
    }
}

#[tokio::test]
#[ignore]
async fn update_touching_only_upstream_does_not_rewrite_the_service_record() {
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
    assert_eq!(before, after, "the service record itself must not be re-written when only its upstream changed");

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
async fn update_not_touching_upstream_does_not_rewrite_the_upstream_record() {
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
    assert_eq!(before, after, "the upstream record must not be re-written when only non-upstream fields changed");

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
