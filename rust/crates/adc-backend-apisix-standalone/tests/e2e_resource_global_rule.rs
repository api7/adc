//! Ported from `libs/backend-apisix-standalone/e2e/resources/global-rule.e2e-spec.ts`.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_backend_core::{HttpClient, HttpClientConfig, Method, TlsConfig};
use adc_sdk::resources::Configuration;
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, ResourceType};
use serde_json::json;

mod common;
use common::{backend, create_event, delete_event, update_event};

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
async fn creates_dumps_updates_and_deletes_global_rules() {
    common::restart_apisix().await;
    let backend = backend("global-rule-e2e");

    // Initialize cache.
    dump(&backend).await;

    let plugin1_name = "request-id";
    let plugin2_name = "prometheus";
    sync_ok(
        &backend,
        vec![
            create_event(ResourceType::GlobalRule, plugin1_name, json!({}), None),
            create_event(ResourceType::GlobalRule, plugin2_name, json!({ "prefer_name": true }), None),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let global_rules = config.global_rules.expect("global rules were just created");
    assert_eq!(global_rules.len(), 2);
    assert_eq!(global_rules.get(plugin1_name), Some(&json!({})));
    assert_eq!(global_rules.get(plugin2_name).and_then(|v| v.get("prefer_name")), Some(&json!(true)));

    // Regression coverage for #489: re-syncing the already-applied state
    // must be recognized as identical (by the digest this crate stamps on
    // every PUT — see `crate::operator::Operator::sync`) and not bump the
    // server's own conf_version, even though a document still gets sent.
    let version_before = raw_global_rules_conf_version().await;
    sync_ok(&backend, vec![]).await;
    let version_after = raw_global_rules_conf_version().await;
    assert_eq!(version_before, version_after, "resyncing unchanged global rules must not bump the conf_version");

    sync_ok(
        &backend,
        vec![update_event(ResourceType::GlobalRule, plugin1_name, json!({ "enable": false }), json!({}), None)],
    )
    .await;

    let config = dump(&backend).await;
    let global_rules = config.global_rules.expect("global rules still exist");
    assert_eq!(global_rules.get(plugin1_name).and_then(|v| v.get("enable")), Some(&json!(false)));
    assert_eq!(global_rules.get(plugin2_name).and_then(|v| v.get("prefer_name")), Some(&json!(true)));

    sync_ok(
        &backend,
        vec![delete_event(ResourceType::GlobalRule, plugin1_name, None), delete_event(ResourceType::GlobalRule, plugin2_name, None)],
    )
    .await;

    let config = dump(&backend).await;
    assert_eq!(config.global_rules.map(|rules| rules.len()).unwrap_or(0), 0);
}

/// Reads `global_rules_conf_version` straight off the admin API — bypasses
/// this crate's own cache entirely, so it reflects what the server actually
/// has, not what we think we last wrote.
async fn raw_global_rules_conf_version() -> Option<i64> {
    let client = HttpClient::new(HttpClientConfig {
        server: common::SERVER1.to_string(),
        token: common::TOKEN.to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let request = client.request(Method::GET, "/apisix/admin/configs").unwrap();
    let body: serde_json::Value = client.send_json(request).await.unwrap();
    body.get("global_rules_conf_version").and_then(|v| v.as_i64())
}
