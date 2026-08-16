//! Ported from `libs/differ/src/test/service-upstream.spec.ts`.

use adc_differ::DifferV4;
use adc_sdk::{EventKind, ResourceType, ValueDiff, utils::generate_id};
use serde_json::json;

mod common;
use common::{config, ev};

#[test]
fn unchanged_service_with_only_default_upstream() {
    let service = json!({ "id": "service1", "name": "service1", "upstream": { "nodes": [{ "host": "upstream1", "port": 80, "weight": 1 }] } });
    let local = config(json!({ "services": [service.clone()] }));
    let remote = config(json!({ "services": [service] }));
    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![]);
}

#[test]
fn unchanged_service_with_default_and_named_upstreams() {
    let service = json!({
        "id": "service1", "name": "service1",
        "upstream": { "nodes": [{ "host": "upstream1", "port": 80, "weight": 1 }] },
        "upstreams": [{ "id": "non-default", "name": "non-default" }],
    });
    let local = config(json!({ "services": [service.clone()] }));
    let remote = config(json!({ "services": [service] }));
    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![]);
}

#[test]
fn updates_default_upstream() {
    let service_name = "service1";
    let upstream_name = "upstream1";
    let service = json!({
        "id": service_name, "name": service_name,
        "upstream": { "nodes": [{ "host": upstream_name, "port": 80, "weight": 1 }] },
    });
    let mut remote_service = service.clone();
    remote_service["upstream"] = json!({ "name": upstream_name, "nodes": [{ "host": upstream_name, "port": 80, "weight": 1 }] });

    let local = config(json!({ "services": [service] }));
    let remote = config(json!({ "services": [remote_service] }));

    let events = DifferV4::diff(&local, &remote, None, None);
    assert_eq!(events.len(), 1);
    let e = &events[0];
    assert_eq!(e.resource_type, ResourceType::Service);
    assert_eq!(e.resource_id, service_name);
    assert_eq!(e.resource_name, service_name);
    let EventKind::Update { diff, .. } = &e.kind else { panic!("expected an update event, got {:?}", e.kind) };
    assert_eq!(
        *diff,
        Some(vec![ValueDiff::Deleted {
            path: vec![adc_sdk::PathSegment::Key("upstream".into()), adc_sdk::PathSegment::Key("name".into())],
            lhs: json!(upstream_name),
        }])
    );
}

#[test]
fn creates_service_and_upstream() {
    let service_name = "service1";
    let upstream1_name = "upstream1";
    let upstream2_name = "upstream2";
    let service = json!({
        "id": service_name, "name": service_name,
        "upstream": { "nodes": [{ "host": upstream1_name, "port": 80, "weight": 1 }] },
        "upstreams": [{ "id": upstream2_name, "name": upstream2_name }],
    });
    let local = config(json!({ "services": [service] }));
    let remote = config(json!({}));

    let service_ev = ev(
        ResourceType::Service,
        EventKind::Create {
            new_value: json!({
                "name": service_name,
                "upstream": { "nodes": [{ "host": upstream1_name, "port": 80, "weight": 1 }] },
                "upstreams": [{ "name": upstream2_name }],
            }),
        },
        service_name,
        service_name,
    );

    let mut upstream_ev =
        ev(ResourceType::Upstream, EventKind::Create { new_value: json!({ "name": upstream2_name }) }, upstream2_name, upstream2_name);
    upstream_ev.parent_id = Some(service_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![service_ev, upstream_ev]);
}

#[test]
fn creates_non_default_upstreams() {
    let service_name = "service1";
    let upstream_name = "upstream1";
    let service = json!({ "id": service_name, "name": service_name, "upstreams": [{ "name": upstream_name }] });
    let mut remote_service = service.clone();
    remote_service.as_object_mut().unwrap().remove("upstreams");

    let local = config(json!({ "services": [service] }));
    let remote = config(json!({ "services": [remote_service] }));

    let mut expected = ev(
        ResourceType::Upstream,
        EventKind::Create { new_value: json!({ "name": upstream_name }) },
        &generate_id(&format!("{service_name}.{upstream_name}")),
        upstream_name,
    );
    expected.parent_id = Some(service_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn replaces_non_default_upstreams() {
    let service_name = "service1";
    let upstream1_name = "upstream1";
    let upstream2_name = "upstream2";
    let service = json!({ "id": service_name, "name": service_name, "upstreams": [{ "name": upstream1_name }] });
    let mut remote_service = service.clone();
    remote_service["upstreams"] = json!([{ "id": upstream2_name, "name": upstream2_name }]);

    let local = config(json!({ "services": [service] }));
    let remote = config(json!({ "services": [remote_service] }));

    let mut del =
        ev(ResourceType::Upstream, EventKind::Delete { old_value: json!({ "name": upstream2_name }) }, upstream2_name, upstream2_name);
    del.parent_id = Some(service_name.to_string());

    let mut create = ev(
        ResourceType::Upstream,
        EventKind::Create { new_value: json!({ "name": upstream1_name }) },
        &generate_id(&format!("{service_name}.{upstream1_name}")),
        upstream1_name,
    );
    create.parent_id = Some(service_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![del, create]);
}

#[test]
fn updates_non_default_upstreams() {
    let service_name = "service1";
    let upstream_name = "upstream1";
    let service = json!({
        "id": service_name, "name": service_name,
        "upstreams": [{ "name": upstream_name, "nodes": [{ "host": upstream_name, "port": 80, "weight": 1 }] }],
    });
    let mut remote_service = service.clone();
    remote_service["upstreams"] = json!([{
        "id": generate_id(&format!("{service_name}.{upstream_name}")),
        "name": upstream_name,
        "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }],
    }]);

    let local = config(json!({ "services": [service] }));
    let remote = config(json!({ "services": [remote_service] }));

    let mut expected = ev(
        ResourceType::Upstream,
        EventKind::Update {
            old_value: json!({ "name": upstream_name, "nodes": [{ "host": "1.1.1.1", "port": 80, "weight": 1 }] }),
            new_value: json!({ "name": upstream_name, "nodes": [{ "host": upstream_name, "port": 80, "weight": 1 }] }),
            diff: Some(vec![ValueDiff::Edit {
                path: vec![
                    adc_sdk::PathSegment::Key("nodes".into()),
                    adc_sdk::PathSegment::Index(0),
                    adc_sdk::PathSegment::Key("host".into()),
                ],
                lhs: json!("1.1.1.1"),
                rhs: json!(upstream_name),
            }]),
        },
        &generate_id(&format!("{service_name}.{upstream_name}")),
        upstream_name,
    );
    expected.parent_id = Some(service_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn deletes_non_default_upstreams() {
    let service_name = "service1";
    let upstream_name = "upstream1";
    let service = json!({
        "id": service_name, "name": service_name,
        "upstreams": [{ "id": generate_id(&format!("{service_name}.{upstream_name}")), "name": upstream_name }],
    });
    let mut local_service = service.clone();
    local_service.as_object_mut().unwrap().remove("upstreams");

    let local = config(json!({ "services": [local_service] }));
    let remote = config(json!({ "services": [service] }));

    let mut expected = ev(
        ResourceType::Upstream,
        EventKind::Delete { old_value: json!({ "name": upstream_name }) },
        &generate_id(&format!("{service_name}.{upstream_name}")),
        upstream_name,
    );
    expected.parent_id = Some(service_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}
