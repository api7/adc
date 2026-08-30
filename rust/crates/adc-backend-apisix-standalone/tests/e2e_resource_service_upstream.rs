//! Ported from `libs/backend-apisix-standalone/e2e/resources/service-upstream.e2e-spec.ts`.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::tests::typing::ADC_UPSTREAM_SERVICE_ID_LABEL;
use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::utils::generate_id;
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::{backend, diff, empty_configuration};

const CACHE_KEY: &str = "service-upstream-e2e";

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn node(host: &str) -> adc::UpstreamNode {
    adc::UpstreamNode { host: host.to_string(), port: 443, weight: 100, priority: 0, metadata: None }
}

fn base_upstream() -> adc::Upstream {
    adc::Upstream {
        id: None,
        name: None,
        description: None,
        labels: None,
        r#type: adc::UpstreamBalancer::RoundRobin,
        hash_on: None,
        key: None,
        checks: None,
        nodes: None,
        scheme: adc::UpstreamScheme::Https,
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

fn service_with_named_upstreams(nd1_host: &str) -> adc::Service {
    let nd1 = adc::Upstream { name: Some("nd-upstream1".to_string()), nodes: Some(vec![node(nd1_host)]), ..base_upstream() };
    let nd2 = adc::Upstream {
        id: Some("nd-upstream2".to_string()),
        name: Some("nd-upstream2".to_string()),
        nodes: Some(vec![node("1.0.0.1")]),
        ..base_upstream()
    };
    adc::Service {
        id: None,
        name: "test".to_string(),
        description: None,
        labels: None,
        upstream: Some(adc::Upstream { nodes: Some(vec![node("httpbin.org")]), ..base_upstream() }),
        upstreams: Some(vec![nd1, nd2]),
        plugins: None,
        path_prefix: None,
        strip_path_prefix: None,
        hosts: None,
        routes: None,
    }
}

async fn assert_original_layout() {
    let raw = common::raw_config().await;
    let service_id = generate_id("test");
    assert_eq!(raw.services[0].id, service_id);
    let upstreams = raw.upstreams;
    assert_eq!(upstreams.len(), 3, "default + 2 named upstreams were written");
    let default_upstream = upstreams.iter().find(|u| u.name == "test").expect("the service's own default upstream");
    let nd1 = upstreams.iter().find(|u| u.name == "nd-upstream1").expect("nd-upstream1");
    let nd2 = upstreams.iter().find(|u| u.name == "nd-upstream2").expect("nd-upstream2");
    assert!(default_upstream.labels.is_none(), "a service's default upstream never carries the service-id bookkeeping label");
    assert_eq!(nd1.labels.as_ref().and_then(|l| l.get(ADC_UPSTREAM_SERVICE_ID_LABEL)), Some(&service_id));
    assert_eq!(nd2.labels.as_ref().and_then(|l| l.get(ADC_UPSTREAM_SERVICE_ID_LABEL)), Some(&service_id));

    let config = common::cache().config(CACHE_KEY).await.expect("sync populated the config cache");
    let services = config.services.expect("service exists");
    assert_eq!(services.len(), 1);
    let named = services[0].upstreams.as_ref().expect("named upstreams are nested under the service");
    assert_eq!(named.len(), 2);
    // The bookkeeping label must not leak into the ADC-facing model.
    assert!(named.iter().all(|u| u.labels.as_ref().is_none_or(|l| !l.contains_key(ADC_UPSTREAM_SERVICE_ID_LABEL))));
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_service_with_multiple_named_upstreams() {
    common::restart_apisix().await;
    let backend = backend(CACHE_KEY);
    dump(&backend).await;

    let service = service_with_named_upstreams("1.1.1.1");
    let before = dump(&backend).await;
    let local = Configuration { services: Some(vec![service.clone()]), ..empty_configuration() };
    let events = diff(&local, &before);
    sync_ok(&backend, events).await;

    assert_original_layout().await;

    // Re-syncing the identical desired state produces no events at all.
    let before = dump(&backend).await;
    let events = diff(&local, &before);
    assert!(events.is_empty(), "an unchanged desired state must diff to no events");
    sync_ok(&backend, events).await;

    assert_original_layout().await;

    let raw_before = common::raw_config().await;
    let nd1_index_before = raw_before.upstreams.iter().find(|u| u.name == "nd-upstream1").unwrap().modified_index;
    let nd2_index_before = raw_before.upstreams.iter().find(|u| u.name == "nd-upstream2").unwrap().modified_index;
    let default_index_before = raw_before.upstreams.iter().find(|u| u.name == "test").unwrap().modified_index;
    let services_conf_version_before = raw_before.services_conf_version;

    // Change nd-upstream1's node host; nd-upstream2 and the default
    // upstream must be untouched.
    let updated_service = service_with_named_upstreams("8.8.8.8");
    let before = dump(&backend).await;
    let events = diff(&Configuration { services: Some(vec![updated_service]), ..empty_configuration() }, &before);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    let upstreams = raw.upstreams;
    assert_eq!(upstreams.len(), 3);
    let nd1 = upstreams.iter().find(|u| u.name == "nd-upstream1").expect("nd-upstream1");
    assert_eq!(
        nd1.labels.as_ref().and_then(|l| l.get(ADC_UPSTREAM_SERVICE_ID_LABEL)),
        Some(&generate_id("test"))
    );
    assert_eq!(nd1.nodes.as_ref().unwrap()[0].host, "8.8.8.8");
    assert!(nd1.modified_index > nd1_index_before, "nd-upstream1's own modifiedIndex must bump");

    let nd2_index_after = upstreams.iter().find(|u| u.name == "nd-upstream2").unwrap().modified_index;
    let default_index_after = upstreams.iter().find(|u| u.name == "test").unwrap().modified_index;
    assert_eq!(nd2_index_after, nd2_index_before, "the unrelated nd-upstream2's modifiedIndex must not move");
    assert_eq!(default_index_after, default_index_before, "the service's own default upstream's modifiedIndex must not move");
    assert_eq!(raw.services_conf_version, services_conf_version_before, "a named-upstream-only change must not bump services_conf_version");
    assert_eq!(raw.upstreams_conf_version, nd1.modified_index, "upstreams_conf_version must reflect nd-upstream1's fresh timestamp");

    let config = common::cache().config(CACHE_KEY).await.unwrap();
    let services = config.services.unwrap();
    let named = services[0].upstreams.as_ref().unwrap();
    assert_eq!(named.len(), 2);
    let named_nd1 = named.iter().find(|u| u.name.as_deref() == Some("nd-upstream1")).expect("nd-upstream1");
    assert!(named_nd1.labels.as_ref().is_none_or(|l| !l.contains_key(ADC_UPSTREAM_SERVICE_ID_LABEL)));
    assert_eq!(named_nd1.nodes.as_ref().unwrap()[0].host, "8.8.8.8");
}
