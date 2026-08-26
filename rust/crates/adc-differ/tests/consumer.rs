//! Ported from `libs/differ/src/test/consumer.spec.ts`.

use adc_differ::DifferV4;
use adc_sdk::{EventKind, ResourceType, utils::generate_id};
use serde_json::json;

mod common;
use common::{config, ev};

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
        EventKind::Delete {
            old_value: json!({ "config": { "key": consumer_name, "secret": changeme }, "name": "delete", "type": "jwt-auth" }),
        },
        &generate_id(&format!("{consumer_name}.delete")),
        "delete",
    );
    del.parent_id = Some(consumer_name.to_string());

    let mut create = ev(
        ResourceType::ConsumerCredential,
        EventKind::Create { new_value: json!({ "config": { "key": consumer_name }, "name": "create", "type": "key-auth" }) },
        &generate_id(&format!("{consumer_name}.create")),
        "create",
    );
    create.parent_id = Some(consumer_name.to_string());

    let mut update = ev(
        ResourceType::ConsumerCredential,
        EventKind::Update {
            old_value: json!({ "config": { "password": changeme, "username": consumer_name }, "name": "update", "type": "basic-auth" }),
            new_value: json!({ "config": { "password": format!("{changeme}.new"), "username": consumer_name }, "name": "update", "type": "basic-auth" }),
            diff: Some(vec![adc_sdk::ValueDiff::Edit {
                path: vec![adc_sdk::PathSegment::Key("config".into()), adc_sdk::PathSegment::Key("password".into())],
                lhs: json!(changeme),
                rhs: json!(format!("{changeme}.new")),
            }]),
        },
        &generate_id(&format!("{consumer_name}.update")),
        "update",
    );
    update.parent_id = Some(consumer_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None), vec![del, create, update]);
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

    let consumer_del = ev(
        ResourceType::Consumer,
        EventKind::Delete {
            old_value: json!({
                "username": consumer_name,
                "credentials": [{
                    "id": generate_id(&format!("{consumer_name}.delete")),
                    "name": "delete",
                    "type": "jwt-auth",
                    "config": { "key": consumer_name, "secret": changeme },
                }],
            }),
        },
        consumer_name,
        consumer_name,
    );

    let mut cred_del = ev(
        ResourceType::ConsumerCredential,
        EventKind::Delete {
            old_value: json!({ "config": { "key": consumer_name, "secret": changeme }, "name": "delete", "type": "jwt-auth" }),
        },
        &generate_id(&format!("{consumer_name}.delete")),
        "delete",
    );
    cred_del.parent_id = Some(consumer_name.to_string());

    assert_eq!(DifferV4::diff(&local, &remote, None), vec![consumer_del, cred_del]);
}
