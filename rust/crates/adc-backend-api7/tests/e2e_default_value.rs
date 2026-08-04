//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_default_value -- --ignored --test-threads=1`.

use adc_sdk::ResourceType;
use semver::Version;
use serde_json::{Value, json};

mod common;
use common::{assert_matches_object, get_default_value, server_version};

fn core_default(
    core: &std::collections::HashMap<ResourceType, Value>,
    resource_type: ResourceType,
) -> Value {
    core.get(&resource_type)
        .unwrap_or_else(|| panic!("no default value present for {resource_type:?}"))
        .clone()
}

#[tokio::test]
#[ignore]
async fn default_value_below_3_6_0() {
    if server_version() >= Version::new(3, 6, 0) {
        eprintln!("skipping: only applies below 3.6.0");
        return;
    }
    let backend = common::backend().await;
    let default_value = get_default_value(&backend).await.unwrap();

    assert_matches_object(
        &core_default(&default_value.core, ResourceType::Service),
        &json!({
            "upstream": {
                "checks": {
                    "active": {
                        "concurrency": 10,
                        "healthy": { "http_statuses": [200, 302], "interval": 1, "successes": 2 },
                        "http_path": "/",
                        "https_verify_certificate": true,
                        "timeout": 1,
                        "type": "http",
                        "unhealthy": { "http_failures": 5, "http_statuses": [429, 404, 500, 501, 502, 503, 504, 505], "interval": 1, "tcp_failures": 2, "timeouts": 3 },
                    },
                    "passive": {
                        "healthy": { "http_statuses": [200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302, 303, 304, 305, 306, 307, 308], "successes": 5 },
                        "type": "http",
                        "unhealthy": { "http_failures": 5, "http_statuses": [429, 500, 503], "tcp_failures": 2, "timeouts": 7 },
                    },
                },
                "discovery_args": {},
                "hash_on": "vars",
                "keepalive_pool": { "idle_timeout": 60, "requests": 1000, "size": 320 },
                "name": "default",
                "nodes": [{ "priority": 0 }],
                "pass_host": "pass",
                "retry_timeout": 0,
                "scheme": "http",
                "timeout": { "connect": 60, "read": 60, "send": 60 },
                "type": "roundrobin",
            },
        }),
    );
    assert_matches_object(
        &core_default(&default_value.core, ResourceType::Ssl),
        &json!({ "client": { "depth": 1 } }),
    );
}

#[tokio::test]
#[ignore]
async fn default_value_from_3_6_0_below_3_8_0() {
    if !(server_version() >= Version::new(3, 6, 0) && server_version() < Version::new(3, 8, 0)) {
        eprintln!("skipping: only applies in [3.6.0, 3.8.0)");
        return;
    }
    let backend = common::backend().await;
    let default_value = get_default_value(&backend).await.unwrap();

    assert_matches_object(
        &core_default(&default_value.core, ResourceType::Service),
        &json!({ "strip_path_prefix": true }),
    );
    assert_matches_object(
        &core_default(&default_value.core, ResourceType::Ssl),
        &json!({ "client": { "depth": 1 } }),
    );
}

#[tokio::test]
#[ignore]
async fn default_value_from_3_8_0() {
    if server_version() < Version::new(3, 8, 0) {
        eprintln!("skipping: only applies from 3.8.0");
        return;
    }
    let backend = common::backend().await;
    let default_value = get_default_value(&backend).await.unwrap();
    let core = &default_value.core;

    assert_matches_object(&core_default(core, ResourceType::Consumer), &json!({}));
    assert_matches_object(
        &core_default(core, ResourceType::ConsumerCredential),
        &json!({}),
    );
    assert_matches_object(&core_default(core, ResourceType::GlobalRule), &json!({}));
    assert_matches_object(
        &core_default(core, ResourceType::PluginMetadata),
        &json!({}),
    );
    assert_matches_object(
        &core_default(core, ResourceType::Route),
        &json!({ "priority": 0, "timeout": { "connect": 60, "read": 60, "send": 60 } }),
    );

    let upstream = json!({
        "discovery_args": {},
        "hash_on": "vars",
        "keepalive_pool": { "idle_timeout": 60, "requests": 1000, "size": 320 },
        "name": "default",
        "nodes": [{ "priority": 0 }],
        "pass_host": "pass",
        "retry_timeout": 0,
        "scheme": "http",
        "timeout": { "connect": 60, "read": 60, "send": 60 },
        "type": "roundrobin",
    });
    assert_matches_object(
        &core_default(core, ResourceType::Service),
        &json!({ "strip_path_prefix": true, "upstream": upstream }),
    );
    assert_matches_object(
        &core_default(core, ResourceType::Ssl),
        &json!({ "certificates": [], "snis": [] }),
    );
    assert_matches_object(&core_default(core, ResourceType::StreamRoute), &json!({}));
    assert_matches_object(
        &core_default(core, ResourceType::InternalStreamService),
        &json!({
            "upstream": {
                "hash_on": "vars",
                "name": "default",
                "nodes": [{ "priority": 0 }],
                "retry_timeout": 0,
                "scheme": "tcp",
                "timeout": { "connect": 60, "read": 60, "send": 60 },
                "type": "roundrobin",
            },
        }),
    );
    assert_matches_object(&core_default(core, ResourceType::Upstream), &upstream);
}
