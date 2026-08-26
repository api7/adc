//! Ported from `libs/backend-apisix-standalone/e2e/resources/consumer.e2e-spec.ts`.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::Configuration;
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, ResourceType};
use serde_json::json;

mod common;
use common::{backend, create_event, delete_event, raw_conf_version, update_event};

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_consumers_with_credentials() {
    common::restart_apisix().await;
    let backend = backend("consumer-e2e");
    dump(&backend).await;

    let consumer_name = "consumer1";
    let cred1_name = "consumer1-key";
    let cred2_name = "consumer1-key2";

    sync_ok(
        &backend,
        vec![
            create_event(ResourceType::Consumer, consumer_name, json!({ "username": consumer_name }), None),
            create_event(
                ResourceType::ConsumerCredential,
                cred1_name,
                json!({ "name": cred1_name, "type": "key-auth", "config": { "key": cred1_name } }),
                Some(consumer_name),
            ),
            create_event(
                ResourceType::ConsumerCredential,
                cred2_name,
                json!({ "name": cred2_name, "type": "key-auth", "config": { "key": cred2_name } }),
                Some(consumer_name),
            ),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let consumers = config.consumers.expect("consumer was just created");
    assert_eq!(consumers.len(), 1);
    assert_eq!(consumers[0].username, consumer_name);
    let credentials = consumers[0].credentials.clone().expect("credentials were just created");
    assert_eq!(credentials.len(), 2);
    assert!(credentials.iter().any(|c| c.name == cred1_name));
    assert!(credentials.iter().any(|c| c.name == cred2_name));

    let version_before_update =
        raw_conf_version("consumers_conf_version").await.expect("consumers_conf_version present after consumer sync");
    sync_ok(
        &backend,
        vec![update_event(
            ResourceType::ConsumerCredential,
            cred1_name,
            json!({ "name": cred1_name, "type": "key-auth", "config": { "key": "new-key" } }),
            json!({ "name": cred1_name, "type": "key-auth", "config": { "key": cred1_name } }),
            Some(consumer_name),
        )],
    )
    .await;
    let version_after_update = raw_conf_version("consumers_conf_version")
        .await
        .expect("consumers_conf_version present after credential update");
    assert!(version_after_update > version_before_update, "updating a credential must bump consumers_conf_version");

    let config = dump(&backend).await;
    let credential1 = config.consumers.as_ref().unwrap()[0]
        .credentials
        .as_ref()
        .unwrap()
        .iter()
        .find(|c| c.name == cred1_name)
        .expect("credential1 still exists");
    assert_eq!(credential1.config.get("key"), Some(&json!("new-key")));

    sync_ok(&backend, vec![delete_event(ResourceType::ConsumerCredential, cred1_name, Some(consumer_name))]).await;

    let config = dump(&backend).await;
    let credentials = config.consumers.as_ref().unwrap()[0].credentials.clone().unwrap();
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].name, cred2_name);

    sync_ok(
        &backend,
        vec![
            delete_event(ResourceType::Consumer, consumer_name, None),
            delete_event(ResourceType::ConsumerCredential, cred2_name, Some(consumer_name)),
        ],
    )
    .await;

    let config = dump(&backend).await;
    assert_eq!(config.consumers.map(|c| c.len()).unwrap_or(0), 0);
}
