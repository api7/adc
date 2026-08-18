//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_resource_consumer -- --ignored --test-threads=1`.

use adc_sdk::ResourceType;
use semver::Version;
use serde_json::json;

mod common;
use common::{
    assert_matches_object, create_event, delete_event, dump_configuration, server_version,
    sync_events, update_event,
};

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_consumers_without_credential_support() {
    if server_version() >= Version::new(3, 2, 15) {
        eprintln!("skipping: only applies below 3.2.15");
        return;
    }
    let backend = common::backend().await;
    let consumer1_name = "consumer1";
    let mut consumer1 =
        json!({ "username": consumer1_name, "plugins": { "key-auth": { "key": consumer1_name } } });
    let consumer2_name = "consumer2";
    let consumer2 =
        json!({ "username": consumer2_name, "plugins": { "key-auth": { "key": consumer2_name } } });

    sync_events(
        &backend,
        vec![
            create_event(
                ResourceType::Consumer,
                consumer1_name,
                consumer1.clone(),
                None,
            ),
            create_event(
                ResourceType::Consumer,
                consumer2_name,
                consumer2.clone(),
                None,
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let mut consumers = dump.consumers.clone().unwrap();
    assert_eq!(consumers.len(), 2);
    consumers.sort_by(|a, b| a.username.cmp(&b.username));
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer1);
    assert_matches_object(&serde_json::to_value(&consumers[1]).unwrap(), &consumer2);

    consumer1["description"] = json!("desc");
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::Consumer,
            consumer1_name,
            consumer1.clone(),
            None,
        )],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let updated_consumer1 = dump
        .consumers
        .unwrap()
        .into_iter()
        .find(|c| c.username == consumer1_name)
        .expect("consumer1 missing from dump");
    assert_matches_object(&serde_json::to_value(&updated_consumer1).unwrap(), &consumer1);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Consumer, consumer1_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let consumers = dump.consumers.as_ref().unwrap();
    assert_eq!(consumers.len(), 1);
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer2);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Consumer, consumer2_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.consumers.is_none_or(|c| c.is_empty()));
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_consumers_with_credential_support() {
    if server_version() < Version::new(3, 2, 15) {
        eprintln!("skipping: only applies from 3.2.15");
        return;
    }
    let backend = common::backend().await;
    let consumer1_name = "consumer1";
    let consumer1_key = "consumer1-key";
    let mut consumer1_cred =
        json!({ "name": consumer1_key, "type": "key-auth", "config": { "key": consumer1_key } });
    let mut consumer1 = json!({ "username": consumer1_name, "credentials": [consumer1_cred] });

    sync_events(
        &backend,
        vec![
            create_event(
                ResourceType::Consumer,
                consumer1_name,
                json!({ "username": consumer1_name }),
                None,
            ),
            create_event(
                ResourceType::ConsumerCredential,
                consumer1_key,
                consumer1_cred.clone(),
                Some(consumer1_name),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let consumers = dump.consumers.as_ref().unwrap();
    assert_eq!(consumers.len(), 1);
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer1);
    assert_matches_object(
        &serde_json::to_value(consumers[0].credentials.as_ref().unwrap()).unwrap(),
        &json!([consumer1_cred]),
    );

    consumer1_cred["config"]["key"] = json!("new-key");
    consumer1["credentials"][0]["config"]["key"] = json!("new-key");
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::ConsumerCredential,
            consumer1_key,
            consumer1_cred.clone(),
            Some(consumer1_name),
        )],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let consumers = dump.consumers.as_ref().unwrap();
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer1);
    assert_eq!(
        consumers[0].credentials.as_ref().unwrap()[0].config["key"],
        json!("new-key")
    );

    sync_events(
        &backend,
        vec![delete_event(
            ResourceType::ConsumerCredential,
            consumer1_key,
            Some(consumer1_name),
        )],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let consumers = dump.consumers.as_ref().unwrap();
    assert_eq!(consumers.len(), 1);
    assert!(consumers[0].credentials.as_ref().unwrap().is_empty());

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Consumer, consumer1_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.consumers.is_none_or(|c| c.is_empty()));
}
