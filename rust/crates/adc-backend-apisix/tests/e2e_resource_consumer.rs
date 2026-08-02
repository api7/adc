//! Ported from `libs/backend-apisix/e2e/resources/consumer.e2e-spec.ts`.
//! Real network calls against a live APISIX (>= 3.11.0, same as the TS
//! suite's own version gate) — see `e2e_apisix.rs`'s module doc for how to
//! bring one up and run this file.

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_apisix::tests::Fetcher;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::Backend as _;
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

fn apisix_version() -> semver::Version {
    std::env::var("BACKEND_APISIX_VERSION").ok().and_then(|v| semver::Version::parse(&v).ok()).unwrap_or(semver::Version::new(999, 999, 999))
}

fn create(rt: ResourceType, id: &str, new_value: serde_json::Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, id, id)
}

fn update(rt: ResourceType, id: &str, new_value: serde_json::Value) -> Event {
    // Consumer credential updates aren't SERVICE events, so the operator
    // doesn't need diff info to decide what to touch — an empty diff is
    // fine here (contrast `e2e_sync_and_dump.rs`'s `update()` helper).
    Event::new(rt, EventKind::Update { old_value: json!({}), new_value, diff: None }, id, id)
}

fn delete(rt: ResourceType, id: &str) -> Event {
    Event::new(rt, EventKind::Delete { old_value: json!({}) }, id, id)
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_consumer_with_a_credential_lifecycle() {
    if apisix_version() < semver::Version::new(3, 11, 0) {
        eprintln!("skipping: consumer credentials require apisix >= 3.11.0");
        return;
    }

    let backend = backend();

    let mut credential = create(
        ResourceType::ConsumerCredential,
        "consumer1-key",
        json!({ "name": "consumer1-key", "type": "key-auth", "config": { "key": "consumer1-key" } }),
    );
    // Apisix keys a credential's parent consumer by literal username in
    // the URL path, not a content hash — matches `main_path`'s special
    // case for `ConsumerCredential` in `operator.rs`.
    credential.parent_id = Some("consumer1".to_string());

    let results = backend
        .sync(vec![create(ResourceType::Consumer, "consumer1", json!({ "username": "consumer1" })), credential], BackendSyncOptions::default())
        .await
        .unwrap();
    for result in &results {
        assert!(result.success, "{:?}", result.error);
    }

    let config = backend.dump().await.unwrap();
    let consumers = config.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    let credentials = consumers[0].credentials.as_ref().expect("consumer should have its credential");
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].config.get("key"), Some(&json!("consumer1-key")));

    let mut updated_credential =
        update(ResourceType::ConsumerCredential, "consumer1-key", json!({ "name": "consumer1-key", "type": "key-auth", "config": { "key": "new-key" } }));
    updated_credential.parent_id = Some("consumer1".to_string());
    let results = backend.sync(vec![updated_credential], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);

    let config = backend.dump().await.unwrap();
    let credentials = config.consumers.unwrap()[0].credentials.clone().unwrap();
    assert_eq!(credentials[0].config.get("key"), Some(&json!("new-key")));

    let mut delete_credential = delete(ResourceType::ConsumerCredential, "consumer1-key");
    delete_credential.parent_id = Some("consumer1".to_string());
    let results = backend.sync(vec![delete_credential], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);

    let config = backend.dump().await.unwrap();
    let consumers = config.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    assert!(consumers[0].credentials.is_none());

    let results = backend.sync(vec![delete(ResourceType::Consumer, "consumer1")], BackendSyncOptions::default()).await.unwrap();
    assert!(results[0].success, "{:?}", results[0].error);

    let config = backend.dump().await.unwrap();
    assert!(config.consumers.is_none() || config.consumers.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn consumer_credentials_are_never_fetched_below_apisix_3_11_0() {
    if apisix_version() < semver::Version::new(3, 11, 0) {
        eprintln!("skipping: needs a real >= 3.11.0 server to prove the client-side gate is what's skipping the fetch, not the server lacking the feature");
        return;
    }

    let backend = backend();
    let mut credential = create(
        ResourceType::ConsumerCredential,
        "gated-key",
        json!({ "name": "gated-key", "type": "key-auth", "config": { "key": "gated-key" } }),
    );
    credential.parent_id = Some("gated-consumer".to_string());
    let results = backend
        .sync(
            vec![create(ResourceType::Consumer, "gated-consumer", json!({ "username": "gated-consumer" })), credential],
            BackendSyncOptions::default(),
        )
        .await
        .unwrap();
    for result in &results {
        assert!(result.success, "{:?}", result.error);
    }

    // Same real server (which genuinely has this consumer's credential —
    // proven above), but the `Fetcher` is told it's talking to a
    // pre-3.11.0 apisix. `list_consumers` must not even attempt the
    // credentials sub-fetch in that case, regardless of what the server
    // could actually return.
    let old_fetcher = Fetcher::new(client(), semver::Version::new(3, 10, 0));
    let consumers = old_fetcher.list_consumers().await.unwrap();
    let consumer = consumers.iter().find(|c| c.username == "gated-consumer").expect("consumer was not found");
    assert!(consumer.credentials.is_none(), "credentials must not be fetched when the fetcher believes the server predates 3.11.0");

    let mut delete_credential = delete(ResourceType::ConsumerCredential, "gated-key");
    delete_credential.parent_id = Some("gated-consumer".to_string());
    let results =
        backend.sync(vec![delete_credential, delete(ResourceType::Consumer, "gated-consumer")], BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}", result.error);
    }
}
