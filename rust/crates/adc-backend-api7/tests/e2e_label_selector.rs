//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_label_selector -- --ignored --test-threads=1`.

use std::collections::HashMap;

use adc_sdk::ResourceType;
use semver::Version;
use serde_json::json;

mod common;
use common::{
    assert_matches_object, backend_with_label_selector, create_event, delete_event,
    dump_configuration, read_asset, server_version, sync_events,
};

fn label_selector(key: &str, value: &str) -> HashMap<String, String> {
    HashMap::from([(key.to_string(), value.to_string())])
}

#[tokio::test]
#[ignore]
async fn dumps_consumers_scoped_by_label_without_credential_support() {
    if server_version() >= Version::new(3, 2, 15) {
        eprintln!("skipping: only applies below 3.2.15");
        return;
    }
    let backend = common::backend().await;
    let consumer1_name = "consumer1";
    let consumer1 = json!({
        "username": consumer1_name,
        "labels": { "team": "1" },
        "plugins": { "key-auth": { "key": consumer1_name } },
    });
    let consumer2_name = "consumer2";
    let consumer2 = json!({
        "username": consumer2_name,
        "labels": { "team": "2" },
        "plugins": { "key-auth": { "key": consumer2_name } },
    });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Consumer, consumer1_name, consumer1.clone(), None),
            create_event(ResourceType::Consumer, consumer2_name, consumer2.clone(), None),
        ],
    )
    .await
    .unwrap();

    let team1_backend = backend_with_label_selector(label_selector("team", "1")).await;
    let dump = dump_configuration(&team1_backend).await.unwrap();
    let consumers = dump.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer1);

    let team2_backend = backend_with_label_selector(label_selector("team", "2")).await;
    let dump = dump_configuration(&team2_backend).await.unwrap();
    let consumers = dump.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer2);

    sync_events(
        &backend,
        vec![
            delete_event(ResourceType::Consumer, consumer1_name, None),
            delete_event(ResourceType::Consumer, consumer2_name, None),
        ],
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn dumps_consumers_scoped_by_label_with_credential_support() {
    if server_version() < Version::new(3, 2, 15) {
        eprintln!("skipping: only applies from 3.2.15");
        return;
    }
    let backend = common::backend().await;
    let consumer1_name = "consumer1";
    let consumer1 = json!({ "username": consumer1_name, "labels": { "team": "1" } });
    let consumer2_name = "consumer2";
    let consumer2 = json!({ "username": consumer2_name, "labels": { "team": "2" } });
    let credential1 = json!({
        "name": "key-1",
        "labels": { "team": "1" },
        "type": "key-auth",
        "config": { "key": "key-1" },
    });
    let credential2 = json!({
        "name": "key-2",
        "labels": { "team": "2" },
        "type": "key-auth",
        "config": { "key": "key-2" },
    });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Consumer, consumer1_name, consumer1.clone(), None),
            create_event(ResourceType::Consumer, consumer2_name, consumer2.clone(), None),
            create_event(
                ResourceType::ConsumerCredential,
                "key-1",
                credential1.clone(),
                Some(consumer1_name),
            ),
            create_event(
                ResourceType::ConsumerCredential,
                "key-2",
                credential2.clone(),
                Some(consumer1_name),
            ),
        ],
    )
    .await
    .unwrap();

    let team1_backend = backend_with_label_selector(label_selector("team", "1")).await;
    let dump = dump_configuration(&team1_backend).await.unwrap();
    let consumers = dump.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer1);
    assert_eq!(consumers[0].credentials.as_ref().unwrap().len(), 2);

    let team2_backend = backend_with_label_selector(label_selector("team", "2")).await;
    let dump = dump_configuration(&team2_backend).await.unwrap();
    let consumers = dump.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    assert_matches_object(&serde_json::to_value(&consumers[0]).unwrap(), &consumer2);

    sync_events(
        &backend,
        vec![
            delete_event(ResourceType::ConsumerCredential, "key-1", Some(consumer1_name)),
            delete_event(ResourceType::Consumer, consumer1_name, None),
            delete_event(ResourceType::Consumer, consumer2_name, None),
        ],
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore]
async fn dumps_ssls_scoped_by_label() {
    let backend = common::backend().await;
    let cert1 = read_asset("certs/test-ssl1.cer").trim().to_string();
    let key1 = read_asset("certs/test-ssl1.key").trim().to_string();
    let cert2 = read_asset("certs/test-ssl2.cer").trim().to_string();
    let key2 = read_asset("certs/test-ssl2.key").trim().to_string();

    let ssl1_snis = ["ssl1-1.com", "ssl1-2.com"];
    let ssl1_name = ssl1_snis.join(",");
    let ssl1 = json!({
        "snis": ssl1_snis,
        "labels": { "team": "1" },
        "certificates": [{ "certificate": cert1, "key": key1 }],
    });
    let mut ssl1_expected = ssl1.clone();
    ssl1_expected["certificates"][0].as_object_mut().unwrap().remove("key");

    let ssl2_snis = ["ssl2-1.com", "ssl2-2.com"];
    let ssl2_name = ssl2_snis.join(",");
    let ssl2 = json!({
        "snis": ssl2_snis,
        "labels": { "team": "2" },
        "certificates": [{ "certificate": cert2, "key": key2 }],
    });
    let mut ssl2_expected = ssl2.clone();
    ssl2_expected["certificates"][0].as_object_mut().unwrap().remove("key");

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Ssl, &ssl1_name, ssl1, None),
            create_event(ResourceType::Ssl, &ssl2_name, ssl2, None),
        ],
    )
    .await
    .unwrap();

    let team1_backend = backend_with_label_selector(label_selector("team", "1")).await;
    let dump = dump_configuration(&team1_backend).await.unwrap();
    let ssls = dump.ssls.unwrap();
    assert_eq!(ssls.len(), 1);
    assert_eq!(ssls[0].certificates[0].key, "");
    assert_matches_object(&serde_json::to_value(&ssls[0]).unwrap(), &ssl1_expected);

    let team2_backend = backend_with_label_selector(label_selector("team", "2")).await;
    let dump = dump_configuration(&team2_backend).await.unwrap();
    let ssls = dump.ssls.unwrap();
    assert_eq!(ssls.len(), 1);
    assert_eq!(ssls[0].certificates[0].key, "");
    assert_matches_object(&serde_json::to_value(&ssls[0]).unwrap(), &ssl2_expected);

    sync_events(
        &backend,
        vec![
            delete_event(ResourceType::Ssl, &ssl1_name, None),
            delete_event(ResourceType::Ssl, &ssl2_name, None),
        ],
    )
    .await
    .unwrap();
}
