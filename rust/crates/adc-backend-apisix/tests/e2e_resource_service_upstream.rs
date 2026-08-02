//! Ported from
//! `libs/backend-apisix/e2e/resources/service-upstream.e2e-spec.ts`. Real
//! network calls against a live APISIX — see `e2e_apisix.rs`'s module doc
//! for how to bring one up and run this file.

use adc_backend_apisix::Backend as ApisixBackend;
use adc_sdk::Backend as _;
use adc_sdk::utils::generate_id;
use adc_sdk::{BackendSyncOptions, Event, EventKind, ResourceType};
use serde_json::json;

mod common;
use common::backend;

fn create(rt: ResourceType, id: &str, new_value: serde_json::Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, id, id)
}

fn create_child(rt: ResourceType, name: &str, new_value: serde_json::Value, parent_name: &str) -> Event {
    let mut event = Event::new(rt, EventKind::Create { new_value }, generate_id(&format!("{parent_name}.{name}")), name);
    event.parent_id = Some(generate_id(parent_name));
    event
}

fn update(rt: ResourceType, id: &str, old_value: serde_json::Value, new_value: serde_json::Value) -> Event {
    let diff = adc_sdk::diff_value(&old_value, &new_value);
    Event::new(rt, EventKind::Update { old_value, new_value, diff }, id, id)
}

fn update_child(rt: ResourceType, name: &str, new_value: serde_json::Value, parent_name: &str) -> Event {
    // Only used for named-upstream updates below, which aren't SERVICE
    // events — the operator doesn't need real diff info to decide what to
    // touch for any type other than SERVICE (see `operator.rs`).
    let mut event = Event::new(rt, EventKind::Update { old_value: json!({}), new_value, diff: None }, generate_id(&format!("{parent_name}.{name}")), name);
    event.parent_id = Some(generate_id(parent_name));
    event
}

fn delete(rt: ResourceType, id: &str) -> Event {
    Event::new(rt, EventKind::Delete { old_value: json!({}) }, id, id)
}

fn delete_child(rt: ResourceType, name: &str, parent_name: &str) -> Event {
    let mut event = Event::new(rt, EventKind::Delete { old_value: json!({}) }, generate_id(&format!("{parent_name}.{name}")), name);
    event.parent_id = Some(generate_id(parent_name));
    event
}

async fn sync_ok(backend: &ApisixBackend, events: Vec<Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?} {}: {:?}", result.event.resource_type, result.event.resource_id, result.error);
    }
}

#[tokio::test]
#[ignore]
async fn service_inline_upstream_lifecycle() {
    let service_name = "test-inline-upstream";
    let backend = backend();
    let service_id = generate_id(service_name);
    let upstream_v1 = json!({ "type": "roundrobin", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] });

    sync_ok(&backend, vec![create(ResourceType::Service, &service_id, json!({ "name": service_name, "upstream": upstream_v1 }))]).await;

    let config = backend.dump().await.unwrap();
    let service = config.services.unwrap().into_iter().find(|s| s.id.as_deref() == Some(&service_id)).unwrap();
    let upstream = service.upstream.as_ref().unwrap();
    assert_eq!(upstream.nodes.as_ref().unwrap().len(), 1);
    assert!(upstream.id.is_none(), "an inlined default upstream must not carry its own id");
    assert!(upstream.name.is_none(), "an inlined default upstream must not carry its own name");

    let upstream_v2 = json!({
        "type": "roundrobin",
        "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 50 }, { "host": "example.com", "port": 80, "weight": 50 }],
    });
    sync_ok(
        &backend,
        vec![update(
            ResourceType::Service,
            &service_id,
            json!({ "name": service_name, "upstream": upstream_v1 }),
            json!({ "name": service_name, "upstream": upstream_v2 }),
        )],
    )
    .await;

    let config = backend.dump().await.unwrap();
    let service = config.services.unwrap().into_iter().find(|s| s.id.as_deref() == Some(&service_id)).unwrap();
    let upstream = service.upstream.as_ref().unwrap();
    assert_eq!(upstream.nodes.as_ref().unwrap().len(), 2);
    assert!(upstream.id.is_none());
    assert!(upstream.name.is_none());

    sync_ok(&backend, vec![delete(ResourceType::Service, &service_id)]).await;
    let config = backend.dump().await.unwrap();
    assert!(config.services.unwrap_or_default().iter().all(|s| s.id.as_deref() != Some(&service_id)));
}

#[tokio::test]
#[ignore]
async fn service_named_upstreams_lifecycle() {
    // A distinct name, not the bare "test" several other e2e files also use
    // — those all currently run as separate sequential test binaries
    // against the one shared APISIX instance, so this doesn't collide
    // today, but a differently-named service here avoids relying on that.
    let service_name = "test-named-upstreams";
    let upstream1_name = "nd-upstream1";
    let upstream2_name = "nd-upstream2";
    let backend = backend();
    let service_id = generate_id(service_name);

    sync_ok(
        &backend,
        vec![
            create(ResourceType::Service, &service_id, json!({ "name": service_name, "upstream": { "type": "roundrobin", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] } })),
            create_child(ResourceType::Upstream, upstream1_name, json!({ "name": upstream1_name, "type": "roundrobin", "scheme": "https", "nodes": [{ "host": "1.1.1.1", "port": 443, "weight": 100 }] }), service_name),
            create_child(ResourceType::Upstream, upstream2_name, json!({ "name": upstream2_name, "type": "roundrobin", "scheme": "https", "nodes": [{ "host": "1.0.0.1", "port": 443, "weight": 100 }] }), service_name),
        ],
    )
    .await;

    let config = backend.dump().await.unwrap();
    let services = config.services.unwrap();
    assert_eq!(services.len(), 1);
    let upstreams = services[0].upstreams.as_ref().expect("service should have its named upstreams");
    assert_eq!(upstreams.len(), 2);
    assert!(upstreams.iter().any(|u| u.name.as_deref() == Some(upstream1_name)));
    assert!(upstreams.iter().any(|u| u.name.as_deref() == Some(upstream2_name)));

    sync_ok(
        &backend,
        vec![update_child(
            ResourceType::Upstream,
            upstream1_name,
            json!({ "name": upstream1_name, "type": "roundrobin", "scheme": "https", "nodes": [{ "host": "1.1.1.1", "port": 443, "weight": 100 }], "retry_timeout": 100 }),
            service_name,
        )],
    )
    .await;

    let config = backend.dump().await.unwrap();
    let upstreams = config.services.unwrap()[0].upstreams.clone().unwrap();
    let updated = upstreams.iter().find(|u| u.name.as_deref() == Some(upstream1_name)).unwrap();
    assert_eq!(updated.retry_timeout, Some(100.0));

    sync_ok(&backend, vec![delete_child(ResourceType::Upstream, upstream2_name, service_name)]).await;
    let config = backend.dump().await.unwrap();
    let upstreams = config.services.unwrap()[0].upstreams.clone().unwrap();
    assert_eq!(upstreams.len(), 1);
    assert_eq!(upstreams[0].name.as_deref(), Some(upstream1_name));

    sync_ok(&backend, vec![delete(ResourceType::Service, &service_id)]).await;
    let config = backend.dump().await.unwrap();
    assert!(config.services.unwrap_or_default().iter().all(|s| s.id.as_deref() != Some(&service_id)));
}
