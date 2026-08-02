//! Ported from `libs/backend-apisix/e2e/misc.e2e-spec.ts`. Real network
//! calls against a live APISIX — see `e2e_apisix.rs`'s module doc for how
//! to bring one up and run this file. Exercises `Backend::dump`'s
//! assembly/nesting logic specifically (a route ending up nested under its
//! service in the dumped `Configuration`), which none of the other e2e
//! files cover — everything else so far only checks the flat, unassembled
//! `Fetcher::list_*` results.

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::Backend as _;
use adc_sdk::{Event, EventKind, ResourceType};
use serde_json::json;

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn backend() -> ApisixBackend {
    let client = HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap();
    ApisixBackend::new(client)
}

fn create(rt: ResourceType, id: &str, new_value: serde_json::Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, id, id)
}

fn delete(rt: ResourceType, id: &str) -> Event {
    Event::new(rt, EventKind::Delete { old_value: json!({}) }, id, id)
}

#[tokio::test]
#[ignore]
async fn syncs_resources_with_custom_ids_and_dump_nests_the_route_under_its_service() {
    let service_id = "custom-service";
    let route_id = "custom-route";
    let backend = backend();

    let service = create(
        ResourceType::Service,
        service_id,
        json!({
            "name": "Test Service",
            "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
        }),
    );
    let mut route = create(ResourceType::Route, route_id, json!({ "name": "Test Route", "uris": ["/test"] }));
    route.parent_id = Some(service_id.to_string());

    let results = backend.sync(vec![service, route], adc_sdk::BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}", result.error);
    }

    let config = backend.dump().await.unwrap();
    let services = config.services.expect("dump should have returned services");
    assert_eq!(services.len(), 1);
    let service = &services[0];
    assert_eq!(service.id.as_deref(), Some(service_id));
    assert_eq!(service.name, "Test Service");

    let routes = service.routes.as_ref().expect("service should have its route nested under it").http().expect("an HTTP route list");
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].id.as_deref(), Some(route_id));
    assert_eq!(routes[0].uris, vec!["/test".to_string()]);

    let delete_route = {
        let mut e = delete(ResourceType::Route, route_id);
        e.parent_id = Some(service_id.to_string());
        e
    };
    let results = backend.sync(vec![delete_route, delete(ResourceType::Service, service_id)], adc_sdk::BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}", result.error);
    }

    let config = backend.dump().await.unwrap();
    assert!(config.services.is_none() || config.services.unwrap().is_empty());
}
