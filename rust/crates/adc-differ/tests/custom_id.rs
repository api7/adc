//! Ported from `libs/differ/src/test/custom-id.spec.ts`.

use adc_differ::DifferV4;
use adc_sdk::{EventKind, ResourceType, utils::generate_id};
use serde_json::json;

mod common;
use common::{config, ev};

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

    let del1 = ev(
        ResourceType::Service,
        EventKind::Delete { old_value: json!({ "name": service1_name }) },
        &service1_id,
        service1_name,
    );
    let del2 = ev(
        ResourceType::Service,
        EventKind::Delete { old_value: json!({ "name": service2_name }) },
        custom_id2,
        service2_name,
    );
    let create1 = ev(
        ResourceType::Service,
        EventKind::Create { new_value: json!({ "name": service1_name }) },
        custom_id1,
        service1_name,
    );
    let create2 = ev(
        ResourceType::Service,
        EventKind::Create { new_value: json!({ "name": service2_name }) },
        &service2_id,
        service2_name,
    );

    assert_eq!(DifferV4::diff(&local, &remote, None), vec![del1, del2, create1, create2]);
}
