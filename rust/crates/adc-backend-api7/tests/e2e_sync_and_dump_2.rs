//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_sync_and_dump_2 -- --ignored --test-threads=1`.

use serde_json::json;

mod common;
use common::{assert_matches_object, dump_configuration, load_events_fixture, sync_events};

/// Syncs a real, fairly large mixed-resource-type fixture and checks the
/// dump back against it, then cleans up with a matching "clean" fixture.
#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_mixed_configuration() {
    let backend = common::backend().await;

    sync_events(&backend, load_events_fixture("mixed-1.json"))
        .await
        .unwrap();

    let mut dump = dump_configuration(&backend).await.unwrap();

    let ssls = dump.ssls.as_ref().unwrap();
    assert!(!ssls.is_empty(), "expected at least one ssl, got none");
    let mut ssl0 = serde_json::to_value(&ssls[0]).unwrap();
    let cert = ssl0["certificates"][0]["certificate"]
        .as_str()
        .unwrap()
        .trim()
        .to_string();
    ssl0["certificates"][0]["certificate"] = json!(cert);
    assert_matches_object(
        &ssl0,
        &json!({
            "type": "server",
            "snis": ["test.com"],
            "certificates": [{ "certificate": "-----BEGIN CERTIFICATE-----\nMIICrTCCAZUCFCcH5+jEDUhpTxEQo/pZYC91e2aYMA0GCSqGSIb3DQEBCwUAMBEx\nDzANBgNVBAMMBlJPT1RDQTAgFw0yNDAxMTgwNjAzMDNaGA8yMTIzMTIyNTA2MDMw\nM1owEzERMA8GA1UEAwwIdGVzdC5jb20wggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAw\nggEKAoIBAQCVkfufMRK2bckdpQ/aRfcaTxmjsv5Mb+sJdhb0QuEuXp/VgN3yzFM0\nzCmAeBZwNKpU3HZDv0tnkTx7OARYpj5Bw1ole0EfPVPKBRjlLE56tabzyd4vdLV2\nbk7jYH+H8NjGZNEYLm9MdWiB4Ulyc0+XFA0ZL5WWKOi+oSQVUibT8QK0CENFKNLP\nQjEXlbyujzRS3u6r99EEEy8+3psBA2EELq8GAjEp+jilWggBhUEpLQxCHhHeNevR\nkg5iEvhOhEVKtr5xvgolg5Wvz7GmDulIW9MCu0dIXim52H/spPwgi3yRraY1XjxU\nREyj5tcY7n7LBESkx/ODXEyCkICIPpo9AgMBAAEwDQYJKoZIhvcNAQELBQADggEB\nADBU5XvbnjaF4rpQoqdzgK6BuRvD/Ih/rh+xc+G9mm+qaHx0g3TdTqyhCvSg6aRW\njDq4Z0NILdb6wmJcunua1jjmOQMXER5y34Xfn21+dzjLN2Bl+/vZ/HyXlCjxkppG\nZAsd1H0/jmXqN1zddIThxOccmRcDEP+9GT3hba50sijFbO30Zx+ORJCoT8he6Kyw\nKdOs/yyukafoAtlpoPR+ao/kumto6w/rLfFlEsehU0dMGNgPVSxxVNtBSdxPTUBk\nD6mfqB4f//2DuAmiO+l5RmPUmumqzcYlpd+oAdy3OSnNEHbgxishZr/GI3s6DmUh\n16bgI69aQ5F+MnN3trvaufc=\n-----END CERTIFICATE-----" }],
        }),
    );

    let mut services = dump.services.take().unwrap();
    assert!(
        services.len() >= 2,
        "expected at least two services, got {}",
        services.len()
    );
    services.sort_by(|a, b| a.name.cmp(&b.name));

    assert_matches_object(
        &serde_json::to_value(&services[0]).unwrap(),
        &json!({
            "name": "service1",
            "description": "service1 description",
            "upstream": {
                "name": "default",
                "scheme": "http",
                "type": "roundrobin",
                "hash_on": "vars",
                "nodes": [{ "host": "host", "port": 1100, "weight": 1100, "priority": 0 }],
                "retry_timeout": 0,
                "pass_host": "pass",
                "checks": {
                    "active": {
                        "type": "tcp",
                        "timeout": 1,
                        "concurrency": 10,
                        "http_path": "/",
                        "healthy": { "interval": 1, "http_statuses": [200, 302], "successes": 2 },
                        "unhealthy": { "interval": 1, "http_statuses": [429, 404, 500, 501, 502, 503, 504, 505], "http_failures": 5, "tcp_failures": 2, "timeouts": 3 },
                    },
                },
            },
            "plugins": {
                "limit-count": {
                    "allow_degradation": false,
                    "count": 2,
                    "key": "$consumer_name $remote_addr",
                    "key_type": "var_combination",
                    "policy": "local",
                    "rejected_code": 503,
                    "show_limit_quota_header": true,
                    "time_window": 60,
                },
            },
        }),
    );

    let mut routes0 = services[0]
        .routes
        .as_ref()
        .unwrap()
        .http()
        .unwrap()
        .to_vec();
    routes0.sort_by(|a, b| a.name.cmp(&b.name));
    assert_matches_object(
        &serde_json::to_value(&routes0[0]).unwrap(),
        &json!({
            "uris": ["/anything"],
            "name": "route1.1",
            "methods": ["GET"],
            "enable_websocket": false,
            "plugins": {
                "limit-count": {
                    "allow_degradation": false,
                    "count": 2,
                    "key": "$consumer_name $remote_addr",
                    "key_type": "var_combination",
                    "policy": "local",
                    "rejected_code": 503,
                    "show_limit_quota_header": true,
                    "time_window": 60,
                },
            },
        }),
    );
    assert_matches_object(
        &serde_json::to_value(&routes0[1]).unwrap(),
        &json!({ "uris": ["/anything"], "name": "route1.2", "methods": ["POST"], "enable_websocket": false }),
    );

    assert_matches_object(
        &serde_json::to_value(&services[1]).unwrap(),
        &json!({
            "name": "service2",
            "description": "service2 description",
            "upstream": {
                "name": "default",
                "scheme": "http",
                "type": "roundrobin",
                "hash_on": "vars",
                "nodes": [{ "host": "host", "port": 1100, "weight": 1100, "priority": 0 }],
                "retry_timeout": 0,
                "pass_host": "pass",
            },
        }),
    );

    let mut routes1 = services[1]
        .routes
        .as_ref()
        .unwrap()
        .http()
        .unwrap()
        .to_vec();
    routes1.sort_by(|a, b| a.name.cmp(&b.name));
    assert_matches_object(
        &serde_json::to_value(&routes1[0]).unwrap(),
        &json!({
            "uris": ["/getSomething"],
            "name": "route2.1",
            "methods": ["GET", "POST"],
            "enable_websocket": false,
            "plugins": {
                "limit-count": {
                    "allow_degradation": false,
                    "count": 2,
                    "key": "$consumer_name $remote_addr",
                    "key_type": "var_combination",
                    "policy": "local",
                    "rejected_code": 503,
                    "show_limit_quota_header": true,
                    "time_window": 60,
                },
            },
        }),
    );
    assert_matches_object(
        &serde_json::to_value(&routes1[1]).unwrap(),
        &json!({ "uris": ["/postSomething"], "name": "route2.2", "methods": ["POST", "PUT"], "enable_websocket": false }),
    );

    assert_matches_object(
        &dump.global_rules.as_ref().unwrap()["prometheus"],
        &json!({ "prefer_name": false }),
    );
    assert_matches_object(
        &dump.plugin_metadata.as_ref().unwrap()["http-logger"],
        &json!({ "log_format": { "@timestamp": "$time_iso8601", "client_ip": "$remote_addr", "host": "$host" } }),
    );
    assert_matches_object(
        &dump.plugin_metadata.as_ref().unwrap()["tcp-logger"],
        &json!({ "log_format": { "@timestamp": "$time_iso8601", "client_ip": "$remote_addr", "host": "$host" } }),
    );

    sync_events(&backend, load_events_fixture("mixed-1-clean.json"))
        .await
        .unwrap();
}

#[test]
fn fixtures_parse_without_a_live_server() {
    let create_events = load_events_fixture("mixed-1.json");
    assert!(!create_events.is_empty());
    let clean_events = load_events_fixture("mixed-1-clean.json");
    assert!(!clean_events.is_empty());
}
