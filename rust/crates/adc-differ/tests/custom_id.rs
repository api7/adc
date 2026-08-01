//! Ported from `libs/differ/src/test/custom-id.spec.ts`.

use adc_differ::DifferV4;
use adc_sdk::{Event, EventType, InternalConfiguration, ResourceType, utils::generate_id};
use serde_json::{Value, json};

fn config(v: Value) -> InternalConfiguration {
    v.as_object().cloned().unwrap_or_default()
}

fn ev(rt: ResourceType, et: EventType, id: &str, name: &str) -> Event {
    Event::new(rt, et, id, name)
}

#[test]
fn deletes_and_creates_new_resource_when_id_changes() {
    let service1_name = "Test Service 1";
    let service1_id = generate_id(service1_name);
    let service2_name = "Test Service 2";
    let service2_id = generate_id(service2_name);
    let custom_id1 = "custom-id-1";
    let custom_id2 = "custom-id-2";

    let local = config(json!({
        "services": [
            { "id": custom_id1, "name": service1_name },
            { "name": service2_name },
        ]
    }));
    let remote = config(json!({
        "services": [
            { "id": service1_id, "name": service1_name },
            { "id": custom_id2, "name": service2_name },
        ]
    }));

    let mut del1 = ev(ResourceType::Service, EventType::Delete, &service1_id, service1_name);
    del1.old_value = Some(json!({ "name": service1_name }));
    let mut del2 = ev(ResourceType::Service, EventType::Delete, custom_id2, service2_name);
    del2.old_value = Some(json!({ "name": service2_name }));
    let mut create1 = ev(ResourceType::Service, EventType::Create, custom_id1, service1_name);
    create1.new_value = Some(json!({ "name": service1_name }));
    let mut create2 = ev(ResourceType::Service, EventType::Create, &service2_id, service2_name);
    create2.new_value = Some(json!({ "name": service2_name }));

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![del1, del2, create1, create2]);
}
