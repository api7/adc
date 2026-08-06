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
use common::{backend, diff, empty_configuration};

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

fn base_upstream() -> adc::Upstream {
    adc::Upstream {
        id: None,
        name: None,
        description: None,
        labels: None,
        r#type: adc::UpstreamBalancer::default(),
        hash_on: None,
        key: None,
        checks: None,
        nodes: None,
        scheme: adc::UpstreamScheme::default(),
        retries: None,
        retry_timeout: None,
        timeout: None,
        tls: None,
        keepalive_pool: None,
        pass_host: adc::UpstreamPassHost::default(),
        upstream_host: None,
        service_name: None,
        discovery_type: None,
        discovery_args: None,
    }
}

fn base_service() -> adc::Service {
    adc::Service {
        id: None,
        name: SERVICE_NAME.to_string(),
        description: None,
        labels: None,
        upstream: None,
        upstreams: None,
        plugins: None,
        path_prefix: None,
        strip_path_prefix: None,
        hosts: None,
        routes: None,
    }
}

fn service_with_nodes(nodes: Vec<adc::UpstreamNode>) -> Configuration {
    let service = adc::Service { upstream: Some(adc::Upstream { nodes: Some(nodes), ..base_upstream() }), ..base_service() };
    Configuration { services: Some(vec![service]), ..empty_configuration() }
}

fn node(port: u32) -> adc::UpstreamNode {
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

    let raw = common::cache().raw_config(CACHE_KEY).unwrap();
    let service_id = raw.services.as_ref().unwrap()[0].id.clone();
    let service_modified_index = raw.services.as_ref().unwrap()[0].modified_index;
    let upstream_modified_index_1 = raw.upstreams.as_ref().unwrap()[0].modified_index;
    assert_eq!(raw.upstreams.as_ref().unwrap()[0].id, service_id, "the inline upstream shares the service's own id");
    assert_eq!(raw.upstreams.as_ref().unwrap()[0].name, SERVICE_NAME);
    assert_eq!(raw.services_conf_version, Some(service_modified_index));
    assert_eq!(raw.upstreams_conf_version, Some(upstream_modified_index_1));
    assert_eq!(raw.consumers_conf_version, None);
    assert_eq!(raw.global_rules_conf_version, None);
    assert_eq!(raw.plugin_metadata_conf_version, None);
    assert_eq!(raw.routes_conf_version, None);
    assert_eq!(raw.ssls_conf_version, None);

    // --- Update: only the inline upstream's port changes. ---
    let before = dump(&backend).await;
    let local = service_with_nodes(vec![node(19080)]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::cache().raw_config(CACHE_KEY).unwrap();
    let upstream_modified_index_2 = raw.upstreams.as_ref().unwrap()[0].modified_index;
    assert_eq!(raw.services.as_ref().unwrap()[0].modified_index, service_modified_index, "service body itself must be untouched");
    assert!(upstream_modified_index_2 > upstream_modified_index_1);
    assert_eq!(raw.services_conf_version, Some(service_modified_index));
    assert_eq!(raw.upstreams_conf_version, Some(upstream_modified_index_2));
    assert_eq!(raw.consumers_conf_version, None);
    assert_eq!(raw.global_rules_conf_version, None);
    assert_eq!(raw.plugin_metadata_conf_version, None);
    assert_eq!(raw.routes_conf_version, None);
    assert_eq!(raw.ssls_conf_version, None);

    // --- Update again: the inline upstream's nodes become empty. ---
    let before = dump(&backend).await;
    let local = service_with_nodes(vec![]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::cache().raw_config(CACHE_KEY).unwrap();
    let upstream_modified_index_3 = raw.upstreams.as_ref().unwrap()[0].modified_index;
    assert_eq!(raw.upstreams.as_ref().unwrap()[0].nodes, Some(vec![]));
    assert_eq!(raw.services.as_ref().unwrap()[0].modified_index, service_modified_index, "service body still untouched");
    assert!(upstream_modified_index_3 > upstream_modified_index_2);
    assert_eq!(raw.services_conf_version, Some(service_modified_index));
    assert_eq!(raw.upstreams_conf_version, Some(upstream_modified_index_3));
    assert_eq!(raw.consumers_conf_version, None);
    assert_eq!(raw.global_rules_conf_version, None);
    assert_eq!(raw.plugin_metadata_conf_version, None);
    assert_eq!(raw.routes_conf_version, None);
    assert_eq!(raw.ssls_conf_version, None);

    // --- Delete: both the service and its inline upstream disappear
    //     together, sharing the same new timestamp. ---
    let before = dump(&backend).await;
    let events = diff(&empty_configuration(), &before);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type(), adc_sdk::EventType::Delete);
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::Service);
    sync_ok(&backend, events).await;

    let raw = common::cache().raw_config(CACHE_KEY).unwrap();
    assert_eq!(raw.upstreams.map(|u| u.len()).unwrap_or(0), 0);
    assert_eq!(raw.services.map(|s| s.len()).unwrap_or(0), 0);
    let final_services_version = raw.services_conf_version.unwrap();
    let final_upstreams_version = raw.upstreams_conf_version.unwrap();
    assert_eq!(final_services_version, final_upstreams_version, "delete bumps both collections in the same sync");
    assert!(final_services_version > service_modified_index);
    assert!(final_upstreams_version > upstream_modified_index_3);
}
