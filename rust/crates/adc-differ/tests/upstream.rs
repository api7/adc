//! Ported from `libs/differ/src/test/upstream.spec.ts`.

use adc_differ::DifferV4;
use adc_sdk::{EventType, ResourceType, utils::generate_id};
use serde_json::json;

mod common;
use common::config;

#[test]
fn creates_and_updates_ssl_before_upstream() {
    let service_name = "test";
    let upstream_name = "upstream-with-client-cert";

    let local = config(json!({
        "services": [{
            "id": service_name,
            "name": service_name,
            "routes": [],
            "upstream": { "type": "roundrobin", "nodes": [{ "host": "127.0.0.1", "port": 80, "weight": 1 }] },
            "upstreams": [{
                "name": upstream_name,
                "type": "roundrobin",
                "nodes": [{ "host": "127.0.0.1", "port": 80, "weight": 1 }],
                "tls": { "client_cert_id": "test" },
            }],
        }],
        "ssls": [
            {
                "id": "test",
                "snis": ["test1.com", "test2.com"],
                "certificates": [{ "certificate": "CERT", "key": "KEY" }],
            },
            {
                "id": "test2",
                "snis": ["test3.com", "test4.com"],
                "certificates": [{ "certificate": "CERT", "key": "KEY" }],
            },
        ]
    }));
    let remote = config(json!({
        "services": [{
            "id": "test",
            "name": "test",
            "routes": [],
            "upstream": { "type": "roundrobin", "nodes": [{ "host": "127.0.0.1", "port": 80, "weight": 1 }] },
        }],
        "ssls": [{
            "id": "test2",
            "snis": ["test3.com", "test4.com", "test5.com"],
            "certificates": [{ "certificate": "CERT", "key": "KEY" }],
        }]
    }));

    let events = DifferV4::diff(&local, &remote, None);
    let summary: Vec<(ResourceType, EventType, String)> =
        events.iter().map(|e| (e.resource_type, e.event_type(), e.resource_id.clone())).collect();

    assert_eq!(
        summary,
        vec![
            (ResourceType::Ssl, EventType::Create, "test".to_string()),
            (ResourceType::Ssl, EventType::Update, "test2".to_string()),
            (
                ResourceType::Upstream,
                EventType::Create,
                generate_id(&format!("{service_name}.{upstream_name}"))
            ),
        ]
    );
}
