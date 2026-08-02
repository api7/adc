//! Ported from `libs/backend-apisix/e2e/resources/upstream.e2e-spec.ts`.
//! Real network calls against a live APISIX — see `e2e_apisix.rs`'s module
//! doc for how to bring one up and run this file.

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::Backend as _;
use adc_sdk::utils::generate_id;
use adc_sdk::{BackendSyncOptions, Event, EventKind, ResourceType};
use serde_json::json;

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn backend() -> ApisixBackend {
    let client = HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap();
    ApisixBackend::new(client)
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_discovery_based_upstream_with_no_nodes() {
    let backend = backend();
    let service_id = generate_id("service1");

    let service = Event::new(
        ResourceType::Service,
        EventKind::Create {
            new_value: json!({
                "name": "service1",
                "upstream": { "scheme": "https", "discovery_type": "kubernetes", "service_name": "test" },
            }),
        },
        service_id.clone(),
        "service1",
    );
    let results = backend.sync(vec![service], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);

    let config = backend.dump().await.unwrap();
    let services = config.services.unwrap();
    assert_eq!(services.len(), 1);
    let upstream = services[0].upstream.as_ref().expect("service should have its (nodeless) default upstream");
    assert!(upstream.nodes.is_none());
    assert_eq!(upstream.discovery_type.as_deref(), Some("kubernetes"));
    assert_eq!(upstream.service_name.as_deref(), Some("test"));

    let delete = Event::new(ResourceType::Service, EventKind::Delete { old_value: json!({}) }, service_id, "service1");
    let results = backend.sync(vec![delete], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);

    let config = backend.dump().await.unwrap();
    assert!(config.services.is_none() || config.services.unwrap().is_empty());
}
