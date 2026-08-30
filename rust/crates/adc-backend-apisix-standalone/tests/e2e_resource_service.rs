//! Ported from `libs/backend-apisix-standalone/e2e/resources/service.e2e-spec.ts`.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, ResourceType};
use serde_json::json;

mod common;
use common::{backend, base_service, base_upstream, create_event, delete_event, diff, empty_configuration, update_event};

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn config_with_services(services: Vec<adc::Service>) -> Configuration {
    Configuration { services: Some(services), ..empty_configuration() }
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_services_with_no_routes() {
    common::restart_apisix().await;
    let backend = backend("service-e2e-empty");
    dump(&backend).await;

    let test_upstream = adc::Upstream {
        description: Some("test upstream".to_string()),
        scheme: adc::UpstreamScheme::Https,
        nodes: Some(vec![adc::UpstreamNode { host: "httpbin.org".to_string(), port: 443, weight: 100, priority: 0, metadata: None }]),
        ..base_upstream()
    };
    let service1 = adc::Service {
        name: "service1".to_string(),
        upstream: Some(test_upstream.clone()),
        hosts: Some(vec!["example1.com".to_string(), "example2.com".to_string()]),
        ..base_service()
    };
    let service2 = adc::Service { name: "service2".to_string(), upstream: Some(test_upstream.clone()), ..base_service() };

    let before = dump(&backend).await;
    let events = diff(&config_with_services(vec![service1.clone(), service2.clone()]), &before);
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    let services = config.services.expect("services were just created");
    assert_eq!(services.len(), 2);
    let dumped1 = services.iter().find(|s| s.name == "service1").expect("service1 exists");
    assert_eq!(dumped1.hosts, service1.hosts);
    assert_eq!(dumped1.upstream.as_ref().map(|u| u.scheme), Some(adc::UpstreamScheme::Https));
    let dumped2 = services.iter().find(|s| s.name == "service2").expect("service2 exists");
    assert_eq!(dumped2.hosts, None);

    let raw = common::raw_config().await;
    let service1_index_before = raw.services.iter().find(|s| s.name == "service1").unwrap().modified_index;
    let service2_index_before = raw.services.iter().find(|s| s.name == "service2").unwrap().modified_index;

    let before = dump(&backend).await;
    let updated_service1 = adc::Service { description: Some("desc".to_string()), ..service1.clone() };
    let events = diff(&config_with_services(vec![updated_service1, service2.clone()]), &before);
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    let services = config.services.expect("services still exist");
    let dumped2 = services.iter().find(|s| s.name == "service2").expect("service2 untouched by service1's update");
    assert_eq!(dumped2.description, None);

    let raw = common::raw_config().await;
    let service1_index_after = raw.services.iter().find(|s| s.name == "service1").unwrap().modified_index;
    let service2_index_after = raw.services.iter().find(|s| s.name == "service2").unwrap().modified_index;
    assert!(service1_index_after > service1_index_before, "updating service1 must bump its own modifiedIndex");
    assert_eq!(service2_index_after, service2_index_before, "the unrelated service2's modifiedIndex must not move");

    sync_ok(&backend, vec![delete_event(ResourceType::Service, "service1", None)]).await;
    let config = dump(&backend).await;
    let services = config.services.expect("service2 remains");
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].name, "service2");

    sync_ok(&backend, vec![delete_event(ResourceType::Service, "service2", None)]).await;
    let config = dump(&backend).await;
    assert_eq!(config.services.map(|s| s.len()).unwrap_or(0), 0);
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_service_with_routes() {
    common::restart_apisix().await;
    let backend = backend("service-e2e-routes");

    let service_name = "test";
    let route1_name = "route1";
    let route2_name = "route2";

    sync_ok(
        &backend,
        vec![
            create_event(
                ResourceType::Service,
                service_name,
                json!({ "name": service_name, "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] } }),
                None,
            ),
            create_event(ResourceType::Route, route1_name, json!({ "name": route1_name, "uris": ["/route1"] }), Some(service_name)),
            create_event(
                ResourceType::Route,
                route2_name,
                json!({ "name": route2_name, "uris": ["/route2"], "plugins": { "key-auth": {} } }),
                Some(service_name),
            ),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let services = config.services.expect("service was just created");
    assert_eq!(services.len(), 1);
    let routes = services[0].routes.as_ref().and_then(adc::ServiceRoutes::http).expect("service has http routes");
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].name, route1_name);
    assert_eq!(routes[0].uris, vec!["/route1".to_string()]);
    assert_eq!(routes[1].name, route2_name);
    assert_eq!(routes[1].uris, vec!["/route2".to_string()]);

    // --- Update route1 only: route2's own modifiedIndex must not move. ---
    let raw = common::raw_config().await;
    let route1_index_before = raw.routes.iter().find(|r| r.name == route1_name).unwrap().modified_index;
    let route2_index_before = raw.routes.iter().find(|r| r.name == route2_name).unwrap().modified_index;

    sync_ok(
        &backend,
        vec![update_event(
            ResourceType::Route,
            route1_name,
            json!({ "name": route1_name, "uris": ["/route1-updated"] }),
            json!({ "name": route1_name, "uris": ["/route1"] }),
            Some(service_name),
        )],
    )
    .await;

    let raw = common::raw_config().await;
    let route1_index_after = raw.routes.iter().find(|r| r.name == route1_name).unwrap().modified_index;
    let route2_index_after = raw.routes.iter().find(|r| r.name == route2_name).unwrap().modified_index;
    assert!(route1_index_after > route1_index_before, "updating route1 must bump its own modifiedIndex");
    assert_eq!(route2_index_after, route2_index_before, "the unrelated route2's modifiedIndex must not move");

    sync_ok(&backend, vec![delete_event(ResourceType::Route, route1_name, Some(service_name))]).await;
    let config = dump(&backend).await;
    let services = config.services.unwrap();
    let routes = services[0].routes.as_ref().and_then(adc::ServiceRoutes::http).expect("route2 remains");
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].name, route2_name);

    sync_ok(
        &backend,
        vec![
            delete_event(ResourceType::Route, route2_name, Some(service_name)),
            delete_event(ResourceType::Service, service_name, None),
        ],
    )
    .await;
    let config = dump(&backend).await;
    assert_eq!(config.services.map(|s| s.len()).unwrap_or(0), 0);
}

#[tokio::test]
#[ignore]
async fn syncs_a_service_with_a_service_discovery_upstream_and_no_static_nodes() {
    common::restart_apisix().await;
    let backend = backend("service-e2e-discovery");

    let registry_name = "consul";
    let service_name = "svc-upstream-sd";
    let service = adc::Service {
        name: service_name.to_string(),
        upstream: Some(adc::Upstream {
            r#type: adc::UpstreamBalancer::RoundRobin,
            discovery_type: Some(registry_name.to_string()),
            service_name: Some(service_name.to_string()),
            ..base_upstream()
        }),
        ..base_service()
    };

    let before = dump(&backend).await;
    let events = diff(&config_with_services(vec![service]), &before);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    let upstreams = raw.upstreams;
    assert_eq!(upstreams.len(), 1, "the service's default upstream was written");
    assert_eq!(upstreams[0].nodes, None, "a discovery-based upstream has no static node list");
    assert_eq!(upstreams[0].discovery_type.as_deref(), Some(registry_name));
    assert_eq!(upstreams[0].service_name.as_deref(), Some(service_name));

    let before = dump(&backend).await;
    let events = diff(&empty_configuration(), &before);
    sync_ok(&backend, events).await;
}
