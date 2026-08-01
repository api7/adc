//! Ported from `libs/differ/src/test/consumer.spec.ts`.

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
fn creates_updates_deletes_consumer_credentials() {
    let consumer_name = "jack";
    let changeme = "changeme";

    let local = config(json!({
        "consumers": [{
            "username": consumer_name,
            "credentials": [
                { "name": "create", "type": "key-auth", "config": { "key": consumer_name } },
                { "name": "update", "type": "basic-auth", "config": { "username": consumer_name, "password": format!("{changeme}.new") } },
            ],
        }]
    }));
    let remote = config(json!({
        "consumers": [{
            "username": consumer_name,
            "credentials": [
                {
                    "id": generate_id(&format!("{consumer_name}.update")),
                    "name": "update",
                    "type": "basic-auth",
                    "config": { "username": consumer_name, "password": changeme },
                },
                {
                    "id": generate_id(&format!("{consumer_name}.delete")),
                    "name": "delete",
                    "type": "jwt-auth",
                    "config": { "key": consumer_name, "secret": changeme },
                },
            ],
        }]
    }));

    let mut del = ev(
        ResourceType::ConsumerCredential,
        EventType::Delete,
        &generate_id(&format!("{consumer_name}.delete")),
        "delete",
    );
    del.parent_id = Some(consumer_name.to_string());
    del.old_value = Some(json!({ "config": { "key": consumer_name, "secret": changeme }, "name": "delete", "type": "jwt-auth" }));

    let mut create = ev(
        ResourceType::ConsumerCredential,
        EventType::Create,
        &generate_id(&format!("{consumer_name}.create")),
        "create",
    );
    create.parent_id = Some(consumer_name.to_string());
    create.new_value = Some(json!({ "config": { "key": consumer_name }, "name": "create", "type": "key-auth" }));

    let mut update = ev(
        ResourceType::ConsumerCredential,
        EventType::Update,
        &generate_id(&format!("{consumer_name}.update")),
        "update",
    );
    update.parent_id = Some(consumer_name.to_string());
    update.diff = Some(vec![adc_sdk::ValueDiff::Edit {
        path: vec![adc_sdk::PathSegment::Key("config".into()), adc_sdk::PathSegment::Key("password".into())],
        lhs: json!(changeme),
        rhs: json!(format!("{changeme}.new")),
    }]);
    update.new_value = Some(json!({ "config": { "password": format!("{changeme}.new"), "username": consumer_name }, "name": "update", "type": "basic-auth" }));
    update.old_value = Some(json!({ "config": { "password": changeme, "username": consumer_name }, "name": "update", "type": "basic-auth" }));

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![del, create, update]);
}

#[test]
fn deletes_consumer_credentials_when_consumer_is_deleted() {
    let consumer_name = "jack";
    let changeme = "changeme";

    let local = config(json!({ "consumers": [] }));
    let remote = config(json!({
        "consumers": [{
            "username": consumer_name,
            "credentials": [{
                "id": generate_id(&format!("{consumer_name}.delete")),
                "name": "delete",
                "type": "jwt-auth",
                "config": { "key": consumer_name, "secret": changeme },
            }],
        }]
    }));

    let mut consumer_del = ev(ResourceType::Consumer, EventType::Delete, consumer_name, consumer_name);
    consumer_del.old_value = Some(json!({
        "username": consumer_name,
        "credentials": [{
            "id": generate_id(&format!("{consumer_name}.delete")),
            "name": "delete",
            "type": "jwt-auth",
            "config": { "key": consumer_name, "secret": changeme },
        }],
    }));

    let mut cred_del = ev(
        ResourceType::ConsumerCredential,
        EventType::Delete,
        &generate_id(&format!("{consumer_name}.delete")),
        "delete",
    );
    cred_del.parent_id = Some(consumer_name.to_string());
    cred_del.old_value = Some(json!({ "config": { "key": consumer_name, "secret": changeme }, "name": "delete", "type": "jwt-auth" }));

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![consumer_del, cred_del]);
}
