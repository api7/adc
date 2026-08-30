//! Ported from
//! `libs/backend-apisix-standalone/e2e/resources/service-inline-upstream.e2e-spec.ts`.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.
//!
//! The TS suite pins `stableTimestamp()` (via `vi.mock`) to exact values
//! (100/200/300/400) and asserts every `modifiedIndex`/`*_conf_version`
//! against them directly. This crate has no clock-injection seam — adding
//! one purely to make a handful of assertions exact would be a production
//! code change made only to serve a test, not because anything real needs
//! it — so this port checks the same underlying invariants a different way:
//! a service's own `modifiedIndex` must stay fixed across an upstream-only
//! update (and only the `upstreams` collection's version moves), and each
//! subsequent write's timestamp must be strictly greater than the last.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::{backend, base_service, base_upstream, diff, empty_configuration};

const CACHE_KEY: &str = "service-inline-upstream-e2e";
const SERVICE_NAME: &str = "test";

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn service_with_nodes(nodes: Vec<adc::UpstreamNode>) -> Configuration {
    let service = adc::Service {
        name: SERVICE_NAME.to_string(),
        upstream: Some(adc::Upstream { nodes: Some(nodes), ..base_upstream() }),
        ..base_service()
    };
    Configuration { services: Some(vec![service]), ..empty_configuration() }
}

fn node(port: u16) -> adc::UpstreamNode {
    adc::UpstreamNode { host: "127.0.0.1".to_string(), port, weight: 100, priority: 0, metadata: None }
}

#[tokio::test]
#[ignore]
async fn a_service_only_update_never_moves_the_services_conf_version_only_upstreams() {
    common::restart_apisix().await;
    let backend = backend(CACHE_KEY);
    dump(&backend).await;

    // --- Create: service with an inline default upstream. ---
    let before = dump(&backend).await;
    let local = service_with_nodes(vec![node(9180)]);
    sync_ok(&backend, diff(&local, &before)).await;

    let raw = common::raw_config().await;
    let service_id = raw.services[0].id.clone();
    let service_modified_index = raw.services[0].modified_index;
    let upstream_modified_index_1 = raw.upstreams[0].modified_index;
    assert_eq!(raw.upstreams[0].id, service_id, "the inline upstream shares the service's own id");
    assert_eq!(raw.upstreams[0].name, SERVICE_NAME);
    assert_eq!(raw.services_conf_version, service_modified_index);
    assert_eq!(raw.upstreams_conf_version, upstream_modified_index_1);
    // Untouched collections are still present, just at their baseline 0.
    assert_eq!(raw.consumers_conf_version, 0);
    assert_eq!(raw.global_rules_conf_version, 0);
    assert_eq!(raw.plugin_metadata_conf_version, 0);
    assert_eq!(raw.routes_conf_version, 0);
    assert_eq!(raw.ssls_conf_version, 0);

    // --- Update: only the inline upstream's port changes. ---
    let before = dump(&backend).await;
    let local = service_with_nodes(vec![node(19080)]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    let upstream_modified_index_2 = raw.upstreams[0].modified_index;
    assert_eq!(raw.services[0].modified_index, service_modified_index, "service body itself must be untouched");
    assert!(upstream_modified_index_2 > upstream_modified_index_1);
    assert_eq!(raw.services_conf_version, service_modified_index);
    assert_eq!(raw.upstreams_conf_version, upstream_modified_index_2);
    assert_eq!(raw.consumers_conf_version, 0);
    assert_eq!(raw.global_rules_conf_version, 0);
    assert_eq!(raw.plugin_metadata_conf_version, 0);
    assert_eq!(raw.routes_conf_version, 0);
    assert_eq!(raw.ssls_conf_version, 0);

    // --- Update again: the inline upstream's nodes become empty. ---
    let before = dump(&backend).await;
    let local = service_with_nodes(vec![]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    let upstream_modified_index_3 = raw.upstreams[0].modified_index;
    assert_eq!(raw.upstreams[0].nodes, Some(vec![]));
    assert_eq!(raw.services[0].modified_index, service_modified_index, "service body still untouched");
    assert!(upstream_modified_index_3 > upstream_modified_index_2);
    assert_eq!(raw.services_conf_version, service_modified_index);
    assert_eq!(raw.upstreams_conf_version, upstream_modified_index_3);
    assert_eq!(raw.consumers_conf_version, 0);
    assert_eq!(raw.global_rules_conf_version, 0);
    assert_eq!(raw.plugin_metadata_conf_version, 0);
    assert_eq!(raw.routes_conf_version, 0);
    assert_eq!(raw.ssls_conf_version, 0);

    // --- Delete: both the service and its inline upstream disappear
    //     together, sharing the same new timestamp. ---
    let before = dump(&backend).await;
    let events = diff(&empty_configuration(), &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type(), adc_sdk::EventType::Delete);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    assert_eq!(raw.upstreams.len(), 0);
    assert_eq!(raw.services.len(), 0);
    let final_services_version = raw.services_conf_version;
    let final_upstreams_version = raw.upstreams_conf_version;
    assert_eq!(final_services_version, final_upstreams_version, "delete bumps both collections in the same sync");
    assert!(final_services_version > service_modified_index);
    assert!(final_upstreams_version > upstream_modified_index_3);
}

fn service_with_host_and_nodes(name: &str, hosts: Option<Vec<String>>, port: u16) -> adc::Service {
    adc::Service { name: name.to_string(), hosts, upstream: Some(adc::Upstream { nodes: Some(vec![node(port)]), ..base_upstream() }), ..base_service() }
}

/// Two independent services, each with its own inline default upstream —
/// covers all four combinations the single-service test above can't (it
/// only ever has one service, so "does an unrelated service's own inline
/// upstream move" has nothing to compare against): changing only service1's
/// own body leaves *both* its own inline upstream and service2 entirely
/// untouched; changing only service1's inline upstream leaves service1's
/// own body and all of service2 untouched; changing neither leaves
/// everything untouched; and (already covered by the test above, not
/// repeated here) changing both moves both.
#[tokio::test]
#[ignore]
async fn two_services_own_inline_upstreams_stay_isolated_from_each_other() {
    common::restart_apisix().await;
    let backend = backend("service-inline-upstream-isolation-e2e");
    dump(&backend).await;

    let before = dump(&backend).await;
    let local = Configuration {
        services: Some(vec![
            service_with_host_and_nodes("svc1", None, 9180),
            service_with_host_and_nodes("svc2", None, 9280),
        ]),
        ..empty_configuration()
    };
    sync_ok(&backend, diff(&local, &before)).await;

    let raw = common::raw_config().await;
    let svc1_id = raw.services.iter().find(|s| s.name == "svc1").unwrap().id.clone();
    let svc2_id = raw.services.iter().find(|s| s.name == "svc2").unwrap().id.clone();
    let svc1_index = raw.services.iter().find(|s| s.name == "svc1").unwrap().modified_index;
    let svc2_index = raw.services.iter().find(|s| s.name == "svc2").unwrap().modified_index;
    let svc1_upstream_index = raw.upstreams.iter().find(|u| u.id == svc1_id).unwrap().modified_index;
    let svc2_upstream_index = raw.upstreams.iter().find(|u| u.id == svc2_id).unwrap().modified_index;

    // --- Scenario: neither changes (pure no-op resync) — nothing moves. ---
    let before = dump(&backend).await;
    let events = diff(&local, &before);
    assert!(events.is_empty(), "an unchanged desired state must diff to no events");
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    assert_eq!(raw.services.iter().find(|s| s.name == "svc1").unwrap().modified_index, svc1_index);
    assert_eq!(raw.services.iter().find(|s| s.name == "svc2").unwrap().modified_index, svc2_index);
    assert_eq!(raw.upstreams.iter().find(|u| u.id == svc1_id).unwrap().modified_index, svc1_upstream_index);
    assert_eq!(raw.upstreams.iter().find(|u| u.id == svc2_id).unwrap().modified_index, svc2_upstream_index);

    // --- Scenario: only service1's own body changes (a `hosts` field, not
    //     `upstream`) — its own inline upstream must not move, and neither
    //     must anything belonging to service2. ---
    let before = dump(&backend).await;
    let local = Configuration {
        services: Some(vec![
            service_with_host_and_nodes("svc1", Some(vec!["svc1.example.com".to_string()]), 9180),
            service_with_host_and_nodes("svc2", None, 9280),
        ]),
        ..empty_configuration()
    };
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    let svc1_index_after = raw.services.iter().find(|s| s.name == "svc1").unwrap().modified_index;
    assert!(svc1_index_after > svc1_index, "changing svc1's own body must bump svc1's own modifiedIndex");
    assert_eq!(
        raw.upstreams.iter().find(|u| u.id == svc1_id).unwrap().modified_index,
        svc1_upstream_index,
        "changing only svc1's body must not move svc1's own inline upstream"
    );
    assert_eq!(raw.services.iter().find(|s| s.name == "svc2").unwrap().modified_index, svc2_index, "svc2 must be untouched");
    assert_eq!(
        raw.upstreams.iter().find(|u| u.id == svc2_id).unwrap().modified_index,
        svc2_upstream_index,
        "svc2's own inline upstream must be untouched by a change to svc1"
    );
    let svc1_index = svc1_index_after;

    // --- Scenario: only service1's inline upstream changes — svc1's own
    //     body, and all of svc2, must not move. ---
    let before = dump(&backend).await;
    let local = Configuration {
        services: Some(vec![
            service_with_host_and_nodes("svc1", Some(vec!["svc1.example.com".to_string()]), 19180),
            service_with_host_and_nodes("svc2", None, 9280),
        ]),
        ..empty_configuration()
    };
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    let svc1_upstream_index_after = raw.upstreams.iter().find(|u| u.id == svc1_id).unwrap().modified_index;
    assert!(svc1_upstream_index_after > svc1_upstream_index, "changing svc1's inline upstream must bump its own modifiedIndex");
    assert_eq!(
        raw.services.iter().find(|s| s.name == "svc1").unwrap().modified_index,
        svc1_index,
        "changing only svc1's inline upstream must not move svc1's own body"
    );
    assert_eq!(raw.services.iter().find(|s| s.name == "svc2").unwrap().modified_index, svc2_index, "svc2 must be untouched");
    assert_eq!(
        raw.upstreams.iter().find(|u| u.id == svc2_id).unwrap().modified_index,
        svc2_upstream_index,
        "svc2's own inline upstream must be untouched by a change to svc1's"
    );
}
