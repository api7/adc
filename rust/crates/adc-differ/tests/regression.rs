//! Rust-side regression tests (not ported from TS) covering bugs found
//! during development of this crate.

use std::collections::HashMap;

use adc_differ::DifferV4;
use adc_sdk::{DefaultValue, InternalConfiguration, ResourceType, utils::generate_id};
use serde_json::{Value, json};

fn config(v: Value) -> InternalConfiguration {
    v.as_object().cloned().unwrap_or_default()
}

/// `resolve_default_type` (SERVICE -> STREAM_SERVICE vs SERVICE) inspects the
/// `stream_routes` field. An earlier optimization pass reordered nested-field
/// stripping to run before default-type resolution, which made this check
/// always see `stream_routes` as absent (already stripped) and silently fall
/// back to plain SERVICE defaults. Locking in the fix: a stream-service-only
/// default must actually apply.
#[test]
fn resolves_stream_service_default_type_correctly() {
    let service_name = "Stream Service";
    let local = config(json!({
        "services": [{
            "name": service_name,
            "stream_routes": [{ "name": "sr1", "server_port": 1234 }],
        }]
    }));
    let remote = config(json!({
        "services": [{
            "id": generate_id(service_name),
            "name": service_name,
            "description": "",
            "stream_routes": [{
                "id": generate_id(&format!("{service_name}.sr1")),
                "name": "sr1",
                "server_port": 1234,
            }],
        }]
    }));
    let default_value = DefaultValue {
        core: HashMap::from([(ResourceType::InternalStreamService, json!({ "description": "" }))]),
        plugins: HashMap::new(),
    };

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![]);
}

/// Sibling check: a plain HTTP service (no stream_routes) must still resolve
/// to SERVICE and NOT pick up a stream-service-only default.
#[test]
fn does_not_apply_stream_service_default_to_http_service() {
    let service_name = "HTTP Service";
    let local = config(json!({ "services": [{ "name": service_name }] }));
    let remote = config(json!({
        "services": [{ "id": generate_id(service_name), "name": service_name, "description": "" }]
    }));
    let default_value = DefaultValue {
        core: HashMap::from([(ResourceType::InternalStreamService, json!({ "description": "" }))]),
        plugins: HashMap::new(),
    };

    let events = DifferV4::diff(&local, &remote, Some(&default_value), None);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, adc_sdk::EventType::Update);
}
