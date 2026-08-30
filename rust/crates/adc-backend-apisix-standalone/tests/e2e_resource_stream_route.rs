//! Mirrors `e2e_resource_service.rs`'s HTTP-route coverage for the
//! stream (TCP/UDP) side, plus the isolation checks that file doesn't make:
//! adding a stream route must bump only `stream_routes`, never `services` —
//! `DifferV4::handle_update` doesn't even emit a `Service` event when the
//! diff is entirely a nested stream_routes change (see its `only_sub_events`
//! branch), so `services_conf_version` has nothing to carry over from but
//! its own unchanged value. Real network calls against a live 3-instance
//! standalone APISIX cluster — see `common`'s module doc for how to bring
//! one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::{backend, base_service, base_upstream, diff, empty_configuration};

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn node(port: u16) -> adc::UpstreamNode {
    adc::UpstreamNode { host: "127.0.0.1".to_string(), port, weight: 100, priority: 0, metadata: None }
}

fn stream_route(name: &str, server_port: u16) -> adc::StreamRoute {
    adc::StreamRoute {
        id: None,
        name: name.to_string(),
        description: None,
        labels: None,
        plugins: None,
        remote_addr: None,
        server_addr: None,
        server_port: Some(server_port),
        sni: None,
    }
}

fn service_with_stream_routes(service_name: &str, upstream_port: u16, routes: Vec<adc::StreamRoute>) -> adc::Service {
    adc::Service {
        name: service_name.to_string(),
        upstream: Some(adc::Upstream { nodes: Some(vec![node(upstream_port)]), ..base_upstream() }),
        routes: Some(adc::ServiceRoutes::Stream { stream_routes: routes }),
        ..base_service()
    }
}

fn config_with_services(services: Vec<adc::Service>) -> Configuration {
    Configuration { services: Some(services), ..empty_configuration() }
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_stream_service_with_stream_routes() {
    // Stream routes are a newer standalone feature — same cutoff
    // `e2e_cache.rs` and `e2e_conf_version_isolation.rs` already check
    // against; nothing to test on a version that doesn't have them at all.
    if common::apisix_version() <= semver::Version::new(3, 13, 0) {
        return;
    }

    common::restart_apisix().await;
    let backend = backend("stream-route-e2e");
    dump(&backend).await;

    let service_name = "stream-svc";

    // --- Create: service + one stream route. ---
    let before = dump(&backend).await;
    let local = config_with_services(vec![service_with_stream_routes(service_name, 19000, vec![stream_route("route1", 9100)])]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 2, "service + stream route");
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    let services = config.services.expect("service was just created");
    assert_eq!(services.len(), 1);
    let stream_routes = services[0].routes.as_ref().and_then(adc::ServiceRoutes::stream).expect("service has stream routes");
    assert_eq!(stream_routes.len(), 1);
    assert_eq!(stream_routes[0].server_port, Some(9100));

    let raw = common::raw_config().await;
    assert_eq!(raw.stream_routes.len(), 1);
    let services_conf_version = raw.services_conf_version;
    let stream_routes_conf_version = raw.stream_routes_conf_version;
    assert_eq!(stream_routes_conf_version, raw.stream_routes[0].modified_index);

    // --- Add a second stream route: must bump only stream_routes,
    //     leave the service itself (and its conf_version) untouched. ---
    let before = dump(&backend).await;
    let local =
        config_with_services(vec![service_with_stream_routes(service_name, 19000, vec![stream_route("route1", 9100), stream_route("route2", 9200)])]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1, "adding a stream route with the service body otherwise unchanged is just one create event");
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::StreamRoute);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    assert_eq!(raw.stream_routes.len(), 2);
    assert_eq!(raw.services_conf_version, services_conf_version, "adding a stream route must not touch services_conf_version");
    assert!(raw.stream_routes_conf_version > stream_routes_conf_version, "adding a stream route must bump stream_routes_conf_version");
    let route1 = raw.stream_routes.iter().find(|r| r.name == "route1").expect("route1 untouched");
    assert_eq!(route1.modified_index, stream_routes_conf_version, "an unrelated existing stream route's modifiedIndex must not move");

    // --- Delete one stream route: the other survives untouched. ---
    sync_ok(&backend, vec![common::delete_event(adc_sdk::ResourceType::StreamRoute, "route1", Some(service_name))]).await;
    let config = dump(&backend).await;
    let services = config.services.unwrap();
    let stream_routes = services[0].routes.as_ref().and_then(adc::ServiceRoutes::stream).expect("route2 remains");
    assert_eq!(stream_routes.len(), 1);
    assert_eq!(stream_routes[0].name, "route2");

    // --- Delete everything. ---
    sync_ok(
        &backend,
        vec![
            common::delete_event(adc_sdk::ResourceType::StreamRoute, "route2", Some(service_name)),
            common::delete_event(adc_sdk::ResourceType::Service, service_name, None),
        ],
    )
    .await;
    let config = dump(&backend).await;
    assert_eq!(config.services.map(|s| s.len()).unwrap_or(0), 0);
    let raw = common::raw_config().await;
    assert_eq!(raw.stream_routes.len(), 0);
}
