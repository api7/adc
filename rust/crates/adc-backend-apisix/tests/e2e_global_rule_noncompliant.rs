//! Real network calls against a live APISIX, see `e2e_apisix.rs`'s module doc
//! for how to bring one up. Lives in its own file so no other test in a shared
//! binary dumps global rules while the non-compliant rule exists.

use adc_backend_core::Method;
use adc_sdk::resources::Configuration;
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, Event, EventKind, ResourceType};
use serde_json::{Value, json};

mod common;
use common::{backend, client};

async fn dump(backend: &adc_backend_apisix::Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn put_raw_global_rule(id: &str, body: Value) {
    let client = client();
    let request = client
        .request(Method::PUT, &format!("/apisix/admin/global_rules/{id}"))
        .unwrap()
        .json(&body);
    client.send(request).await.unwrap();
}

#[tokio::test]
#[ignore]
async fn dumps_noncompliant_global_rules_as_placeholders_and_deletes_them() {
    let backend = backend();
    let multi_id = "legacy-rules";
    let renamed_id = "renamed-rule";
    put_raw_global_rule(
        multi_id,
        json!({ "plugins": { "request-id": {}, "file-logger": { "path": "logs/file.log" } } }),
    )
    .await;
    put_raw_global_rule(renamed_id, json!({ "plugins": { "gzip": {} } })).await;

    let rules = dump(&backend).await.global_rules.expect("rules were just created");
    assert_eq!(rules.len(), 2);
    assert_eq!(
        rules[multi_id]["plugins"]["file-logger"]["path"],
        json!("logs/file.log")
    );
    assert!(rules[multi_id]["plugins"]["request-id"].is_object());
    assert!(rules[renamed_id]["plugins"]["gzip"].is_object());

    // The delete the differ emits for a placeholder key the local config lacks.
    let events: Vec<Event> = [multi_id, renamed_id]
        .into_iter()
        .map(|id| {
            Event::new(
                ResourceType::GlobalRule,
                EventKind::Delete { old_value: rules[id].clone() },
                id,
                id,
            )
        })
        .collect();
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}", result.error);
    }

    let after = dump(&backend).await.global_rules;
    assert!(after.is_none_or(|r| r.is_empty()), "non-compliant rules must be gone");
}
