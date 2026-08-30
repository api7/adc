//! No TS reference spec to port from (`libs/backend-apisix-standalone/e2e/
//! resources/` has no `plugin-metadata.e2e-spec.ts`) — this is a
//! from-scratch addition, structured like `e2e_resource_global_rule.rs`
//! (same `Record`-kind wire shape) but going through the real differ
//! (`common::diff`) instead of hand-built events, and adding the
//! per-entry isolation checks that file doesn't make. Also the e2e
//! counterpart to `transformer.rs`'s unit test for the `modifiedIndex`
//! phantom-update fix: re-diffing right after a sync must produce zero
//! events, not just an unchanged conf_version after a hand-fed empty sync.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;
use serde_json::json;

mod common;
use common::{backend, diff, empty_configuration};

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn config_with_plugin_metadata(entries: Vec<(&str, serde_json::Value)>) -> Configuration {
    let mut map = adc::Plugins::new();
    for (name, value) in entries {
        map.insert(name.to_string(), value);
    }
    Configuration { plugin_metadata: Some(map), ..empty_configuration() }
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_plugin_metadata_without_phantom_updates() {
    common::restart_apisix().await;
    let backend = backend("plugin-metadata-e2e");
    dump(&backend).await;

    // --- Create ---
    let before = dump(&backend).await;
    let local = config_with_plugin_metadata(vec![("http-logger", json!({ "log_format": { "host": "$host" } }))]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    let metadata = config.plugin_metadata.expect("plugin metadata was just created");
    assert_eq!(metadata.get("http-logger").and_then(|v| v.get("log_format")).and_then(|v| v.get("host")), Some(&json!("$host")));

    let raw = common::raw_config().await;
    let http_logger_index = raw.plugin_metadata.iter().find(|p| p.id == "http-logger").unwrap().modified_index;
    assert_eq!(raw.plugin_metadata_conf_version, http_logger_index);

    // --- Regression: re-diffing the just-synced state against a fresh dump
    //     must find nothing to do. `modifiedIndex` living only on the wire
    //     side (never modelled into `Configuration.plugin_metadata`) is what
    //     makes this possible — see `to_adc_omits_plugin_metadata_modified_index`
    //     for the unit-level version of this same guarantee. ---
    let after = dump(&backend).await;
    let noop_events = diff(&local, &after);
    assert!(noop_events.is_empty(), "re-diffing an unchanged plugin_metadata must find zero events, not a phantom update");

    // --- Add a second plugin's metadata: must bump plugin_metadata_conf_version
    //     and the new entry's own modifiedIndex, but leave http-logger's
    //     entry — content and modifiedIndex alike — untouched. ---
    let before = dump(&backend).await;
    let local = config_with_plugin_metadata(vec![
        ("http-logger", json!({ "log_format": { "host": "$host" } })),
        ("prometheus", json!({ "prefer_name": true })),
    ]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1, "only prometheus is new");
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    assert_eq!(raw.plugin_metadata.len(), 2);
    let http_logger = raw.plugin_metadata.iter().find(|p| p.id == "http-logger").expect("http-logger untouched");
    assert_eq!(http_logger.modified_index, http_logger_index, "an unrelated existing plugin's modifiedIndex must not move");
    let prometheus = raw.plugin_metadata.iter().find(|p| p.id == "prometheus").expect("prometheus was just created");
    assert!(prometheus.modified_index > http_logger_index);
    assert_eq!(raw.plugin_metadata_conf_version, prometheus.modified_index);

    // --- Delete both. ---
    let before = dump(&backend).await;
    let events = diff(&empty_configuration(), &before);
    assert_eq!(events.len(), 2);
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    assert_eq!(config.plugin_metadata.map(|m| m.len()).unwrap_or(0), 0);
}
