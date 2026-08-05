//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_validate -- --ignored --test-threads=1`.

use adc_sdk::resources::Configuration;
use adc_sdk::{Event, EventKind, ResourceType};
use semver::Version;
use serde_json::json;

mod common;
use common::{dump_configuration, server_version};

fn config(json: serde_json::Value) -> Configuration {
    serde_json::from_value(json).unwrap()
}

/// `Differ::diff` against an empty remote config: every resource in
/// `config` becomes a `Create` event, a shortcut for building test events
/// without hand-writing each one.
fn config_to_events(cfg: &Configuration) -> Vec<Event> {
    common::diff(cfg, &config(json!({})), None)
}

#[tokio::test]
#[ignore]
async fn reports_unsupported_below_the_minimum_version() {
    if server_version() >= Version::new(3, 9, 10) {
        eprintln!("skipping: only applies below 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let error = backend.validate(&[]).await.unwrap_err();
    assert!(error.to_string().contains("not supported"), "{error}");
}

#[tokio::test]
#[ignore]
async fn succeeds_with_an_empty_configuration() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let result = backend.validate(&[]).await.unwrap();
    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn succeeds_with_a_valid_service_and_route() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let cfg = config(json!({
        "services": [{
            "name": "validate-test-svc",
            "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] },
            "routes": [{ "name": "validate-test-route", "uris": ["/validate-test"], "methods": ["GET"] }],
        }],
    }));

    let result = backend.validate(&config_to_events(&cfg)).await.unwrap();
    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn succeeds_with_a_valid_consumer() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let cfg = config(
        json!({ "consumers": [{ "username": "validate-test-consumer", "plugins": { "key-auth": { "key": "test-key-123" } } }] }),
    );

    let result = backend.validate(&config_to_events(&cfg)).await.unwrap();
    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn fails_with_an_invalid_plugin_configuration() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let cfg = config(json!({
        "services": [{
            "name": "validate-bad-plugin-svc",
            "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] },
            // missing required fields: count, time_window
            "routes": [{ "name": "validate-bad-plugin-route", "uris": ["/bad-plugin"], "plugins": { "limit-count": {} } }],
        }],
    }));

    let result = backend.validate(&config_to_events(&cfg)).await.unwrap();
    assert!(!result.success);
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].resource_type, "routes");
}

/// A route whose `uris` isn't an array of strings fails to deserialize into
/// `adc_sdk::resources::Route` client-side, before any request reaches the
/// server — `validate` surfaces this as an `Err`, not an `Ok` result with
/// `success: false`.
#[tokio::test]
#[ignore]
async fn rejects_a_route_with_a_malformed_uri_client_side() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let event = Event::new(
        ResourceType::Route,
        EventKind::Create {
            new_value: json!({ "name": "validate-bad-route", "uris": [123] }),
        },
        adc_sdk::utils::generate_id("validate-bad-route"),
        "validate-bad-route",
    );

    let error = backend.validate(&[event]).await.unwrap_err();
    assert!(
        matches!(error, adc_sdk::BackendError::Serialization(_)),
        "{error:?}"
    );
}

#[tokio::test]
#[ignore]
async fn collects_multiple_errors() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let cfg = config(json!({
        "services": [{
            "name": "validate-multi-err-svc",
            "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] },
            "routes": [
                { "name": "validate-multi-err-route1", "uris": ["/multi-err-1"], "plugins": { "limit-count": {} } },
                { "name": "validate-multi-err-route2", "uris": ["/multi-err-2"], "plugins": { "limit-count": {} } },
            ],
        }],
    }));

    let result = backend.validate(&config_to_events(&cfg)).await.unwrap();
    assert!(!result.success);
    assert!(result.errors.len() >= 2);
}

#[tokio::test]
#[ignore]
async fn succeeds_with_mixed_resource_types() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let cfg = config(json!({
        "services": [{
            "name": "validate-mixed-svc",
            "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
            "routes": [{ "name": "validate-mixed-route", "uris": ["/mixed-test"], "methods": ["GET", "POST"] }],
        }],
        "consumers": [{ "username": "validate-mixed-consumer", "plugins": { "key-auth": { "key": "mixed-key-456" } } }],
        "global_rules": { "prometheus": { "prefer_name": false } },
    }));

    let result = backend.validate(&config_to_events(&cfg)).await.unwrap();
    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn is_a_dry_run_with_no_side_effects_on_the_server() {
    if server_version() < Version::new(3, 9, 10) {
        eprintln!("skipping: only applies from 3.9.10");
        return;
    }
    use adc_sdk::Backend as _;
    let backend = common::backend().await;
    let service_name = "validate-dryrun-svc";
    let cfg = config(json!({
        "services": [{
            "name": service_name,
            "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] },
            "routes": [{ "name": "validate-dryrun-route", "uris": ["/dryrun-test"] }],
        }],
    }));

    let result = backend.validate(&config_to_events(&cfg)).await.unwrap();
    assert!(result.success);

    let dump = dump_configuration(&backend).await.unwrap();
    assert!(
        dump.services
            .unwrap_or_default()
            .iter()
            .all(|s| s.name != service_name)
    );
}
