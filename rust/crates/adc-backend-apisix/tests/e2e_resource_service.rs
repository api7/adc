//! Ported from `libs/backend-apisix/e2e/resources/service.e2e-spec.ts`.
//! Real network calls against a live APISIX — see `e2e_apisix.rs`'s module
//! doc for how to bring one up and run this file.

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_apisix::tests::Fetcher;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::Backend as _;
use adc_sdk::utils::generate_id;
use adc_sdk::{BackendSyncOptions, Event, EventKind, ResourceType};
use serde_json::json;

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn client() -> HttpClient {
    HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap()
}

fn backend() -> ApisixBackend {
    ApisixBackend::new(client())
}

#[tokio::test]
#[ignore]
async fn creating_a_service_with_an_inline_upstream_splits_it_into_a_separate_resource() {
    let service_name = "test";
    let backend = backend();
    let service_id = generate_id(service_name);

    let service = Event::new(
        ResourceType::Service,
        EventKind::Create { new_value: json!({ "name": service_name, "upstream": { "type": "roundrobin", "nodes": [{ "host": "127.0.0.1", "port": 8080, "weight": 1 }] } }) },
        service_id.clone(),
        service_name,
    );
    let results = backend.sync(vec![service], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);

    let fetcher = Fetcher::new(client(), semver::Version::new(3, 17, 0));
    let services = fetcher.list_services().await.unwrap();
    let wire_service = services.iter().find(|s| s.id == service_id).expect("service was not created");
    assert!(wire_service.upstream_id.is_some(), "service should reference a separate upstream resource by id");
    assert!(wire_service.upstream.is_none(), "the upstream must not be inlined into the service's own wire body");

    let delete = Event::new(ResourceType::Service, EventKind::Delete { old_value: json!({}) }, service_id, service_name);
    let results = backend.sync(vec![delete], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);
}
