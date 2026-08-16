//! Ported from `libs/differ/src/test/usecase.spec.ts`.

use adc_differ::DifferV4;
use adc_sdk::{DefaultValue, EventKind, ResourceType, utils::generate_id};
use serde_json::json;
use std::collections::HashMap;

mod common;
use common::{config, ev};

#[test]
fn renames_service_with_nested_routes() {
    let local = config(json!({
        "services": [{
            "name": "HTTPBIN Service1",
            "routes": [
                { "name": "Anything", "methods": ["GET"], "uris": ["/anything"] },
                { "name": "Generate UUID", "methods": ["GET"], "uris": ["/uuid"] },
            ],
            "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 1, "priority": 0 }] },
        }]
    }));
    let remote = config(json!({
        "services": [{
            "id": generate_id("HTTPBIN Service"),
            "name": "HTTPBIN Service",
            "description": "",
            "routes": [
                { "id": generate_id("HTTPBIN Service.Anything"), "name": "Anything", "methods": ["GET"], "uris": ["/anything"] },
                { "id": generate_id("HTTPBIN Service.Generate UUID"), "name": "Generate UUID", "methods": ["GET"], "uris": ["/uuid"] },
            ],
            "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 1, "priority": 0 }] },
        }]
    }));

    let old_service_id = generate_id("HTTPBIN Service");
    let new_service_id = generate_id("HTTPBIN Service1");

    let mut del_route1 = ev(
        ResourceType::Route,
        EventKind::Delete { old_value: json!({ "methods": ["GET"], "name": "Anything", "uris": ["/anything"] }) },
        &generate_id("HTTPBIN Service.Anything"),
        "Anything",
    );
    del_route1.parent_id = Some(old_service_id.clone());

    let mut del_route2 = ev(
        ResourceType::Route,
        EventKind::Delete { old_value: json!({ "methods": ["GET"], "name": "Generate UUID", "uris": ["/uuid"] }) },
        &generate_id("HTTPBIN Service.Generate UUID"),
        "Generate UUID",
    );
    del_route2.parent_id = Some(old_service_id.clone());

    let del_service = ev(
        ResourceType::Service,
        EventKind::Delete {
            old_value: json!({
                "description": "",
                "name": "HTTPBIN Service",
                "routes": [
                    { "id": generate_id("HTTPBIN Service.Anything"), "methods": ["GET"], "name": "Anything", "uris": ["/anything"] },
                    { "id": generate_id("HTTPBIN Service.Generate UUID"), "name": "Generate UUID", "methods": ["GET"], "uris": ["/uuid"] },
                ],
                "upstream": { "nodes": [{ "host": "httpbin.org", "port": 80, "priority": 0, "weight": 1 }], "scheme": "http" },
            }),
        },
        &old_service_id,
        "HTTPBIN Service",
    );

    let create_service = ev(
        ResourceType::Service,
        EventKind::Create {
            new_value: json!({
                "name": "HTTPBIN Service1",
                "routes": [
                    { "methods": ["GET"], "name": "Anything", "uris": ["/anything"] },
                    { "name": "Generate UUID", "methods": ["GET"], "uris": ["/uuid"] },
                ],
                "upstream": { "nodes": [{ "host": "httpbin.org", "port": 80, "priority": 0, "weight": 1 }], "scheme": "http" },
            }),
        },
        &new_service_id,
        "HTTPBIN Service1",
    );

    let mut create_route1 = ev(
        ResourceType::Route,
        EventKind::Create { new_value: json!({ "methods": ["GET"], "name": "Anything", "uris": ["/anything"] }) },
        &generate_id("HTTPBIN Service1.Anything"),
        "Anything",
    );
    create_route1.parent_id = Some(new_service_id.clone());

    let mut create_route2 = ev(
        ResourceType::Route,
        EventKind::Create { new_value: json!({ "methods": ["GET"], "name": "Generate UUID", "uris": ["/uuid"] }) },
        &generate_id("HTTPBIN Service1.Generate UUID"),
        "Generate UUID",
    );
    create_route2.parent_id = Some(new_service_id.clone());

    assert_eq!(
        DifferV4::diff(&local, &remote, None, None),
        vec![del_route1, del_route2, del_service, create_service, create_route1, create_route2]
    );
}

#[test]
fn selectively_merges_objects_in_default_values_on_a_service() {
    let local = config(json!({
        "services": [{
            "name": "Test Service",
            "upstream": { "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] },
            "routes": [{ "name": "anything", "uris": ["/anything"] }],
        }]
    }));
    let remote = config(json!({
        "services": [{
            "id": generate_id("Test Service"),
            "name": "Test Service",
            "upstream": {
                "name": "default",
                "scheme": "http",
                "type": "roundrobin",
                "hash_on": "vars",
                "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100, "priority": 0 }],
                "retry_timeout": 0,
                "pass_host": "pass",
            },
            "routes": [{ "id": generate_id("Test Service.anything"), "name": "anything", "uris": ["/anything"] }],
        }]
    }));

    let checks = json!({
        "active": {
            "concurrency": 10,
            "healthy": { "http_statuses": [200, 302], "interval": 1, "successes": 2 },
            "http_path": "/",
            "https_verify_certificate": true,
            "timeout": 1,
            "type": "http",
            "unhealthy": {
                "http_failures": 5,
                "http_statuses": [429, 404, 500, 501, 502, 503, 504, 505],
                "interval": 1,
                "tcp_failures": 2,
                "timeouts": 3,
            },
        },
        "passive": {
            "healthy": {
                "http_statuses": [200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302, 303, 304, 305, 306, 307, 308],
                "successes": 5,
            },
            "type": "http",
            "unhealthy": { "http_failures": 5, "http_statuses": [429, 500, 503], "tcp_failures": 2, "timeouts": 7 },
        },
    });
    let default_value = DefaultValue {
        core: HashMap::from([(
            ResourceType::Service,
            json!({
                "upstream": {
                    "checks": checks,
                    "discovery_args": {},
                    "hash_on": "vars",
                    "keepalive_pool": { "idle_timeout": 60, "requests": 1000, "size": 320 },
                    "name": "default",
                    "pass_host": "pass",
                    "retry_timeout": 0,
                    "scheme": "http",
                    "timeout": { "connect": 60, "read": 60, "send": 60 },
                    "type": "roundrobin",
                    "nodes": [{ "priority": 0 }],
                }
            }),
        )]),
        plugins: HashMap::new(),
    };

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![]);
}
