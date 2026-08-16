//! Ported from `libs/differ/src/test/basic.spec.ts` (V4 cases only — this
//! crate only implements the differ v4 algorithm, not the older v3).

use std::collections::HashMap;

use adc_differ::DifferV4;
use adc_sdk::{DefaultValue, EventKind, ResourceType, utils::generate_id};
use serde_json::json;

mod common;
use common::{config, ev};

#[test]
fn empty_input_yields_empty_output() {
    assert_eq!(DifferV4::diff(&config(json!({})), &config(json!({})), None, None), vec![]);
}

#[test]
fn create_resource() {
    let name = "alice";
    let local = config(json!({ "consumers": [{ "username": name, "plugins": {} }] }));
    let remote = config(json!({}));

    let expected =
        ev(ResourceType::Consumer, EventKind::Create { new_value: json!({ "username": name, "plugins": {} }) }, name, name);

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn update_resource() {
    let name = "alice";
    let key = "alice-key";
    let local = config(json!({ "consumers": [{ "username": name, "plugins": { "key-auth": { "key": key } } }] }));
    let remote = config(json!({ "consumers": [{ "username": name, "plugins": {} }] }));

    let expected = ev(
        ResourceType::Consumer,
        EventKind::Update {
            old_value: json!({ "username": name, "plugins": {} }),
            new_value: json!({ "username": name, "plugins": { "key-auth": { "key": key } } }),
            diff: Some(vec![adc_sdk::ValueDiff::New {
                path: vec![adc_sdk::PathSegment::Key("plugins".into()), adc_sdk::PathSegment::Key("key-auth".into())],
                rhs: json!({ "key": key }),
            }]),
        },
        name,
        name,
    );

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn delete_resource() {
    let name = "alice";
    let local = config(json!({}));
    let remote = config(json!({ "consumers": [{ "username": name, "plugins": {} }] }));

    let expected =
        ev(ResourceType::Consumer, EventKind::Delete { old_value: json!({ "username": name, "plugins": {} }) }, name, name);

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn sorted_by_event_type() {
    let created = "createConsumer";
    let updated = "updatedConsumer";
    let deleted = "deletedConsumer";
    let local = config(json!({
        "consumers": [
            { "username": created, "plugins": {} },
            { "username": updated, "plugins": { "key-auth": {} } },
        ]
    }));
    let remote = config(json!({
        "consumers": [
            { "username": updated, "plugins": {} },
            { "username": deleted, "plugins": {} },
        ]
    }));

    let deleted_ev = ev(
        ResourceType::Consumer,
        EventKind::Delete { old_value: json!({ "plugins": {}, "username": deleted }) },
        deleted,
        deleted,
    );

    let updated_ev = ev(
        ResourceType::Consumer,
        EventKind::Update {
            old_value: json!({ "plugins": {}, "username": updated }),
            new_value: json!({ "plugins": { "key-auth": {} }, "username": updated }),
            diff: Some(vec![adc_sdk::ValueDiff::New {
                path: vec![adc_sdk::PathSegment::Key("plugins".into()), adc_sdk::PathSegment::Key("key-auth".into())],
                rhs: json!({}),
            }]),
        },
        updated,
        updated,
    );

    let created_ev = ev(
        ResourceType::Consumer,
        EventKind::Create { new_value: json!({ "plugins": {}, "username": created }) },
        created,
        created,
    );

    // DELETE > UPDATE > CREATE
    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![deleted_ev, updated_ev, created_ev]);
}

#[test]
fn adapts_to_default_core_values() {
    let name = "alice";
    let local = config(json!({ "consumers": [{ "username": name, "plugins": {} }] }));
    let remote = config(json!({ "consumers": [{ "username": name, "description": "", "plugins": {} }] }));
    let default_value = DefaultValue {
        core: HashMap::from([(ResourceType::Consumer, json!({ "description": "" }))]),
        plugins: HashMap::new(),
    };

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![]);
}

#[test]
fn adapts_to_default_plugin_values() {
    let name = "alice";
    let local = config(json!({ "consumers": [{ "username": name, "plugins": { "key-auth": { "key": "key" } } }] }));
    let remote =
        config(json!({ "consumers": [{ "username": name, "plugins": { "key-auth": { "key": "key", "added": "added" } } }] }));
    let default_value = DefaultValue {
        core: HashMap::new(),
        plugins: HashMap::from([("key-auth".to_string(), json!({ "added": "added" }))]),
    };

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![]);
}

#[test]
fn update_resource_add_plugin() {
    let name = "alice";
    let key = "alice-key";
    let local = config(json!({ "consumers": [{ "username": name, "plugins": { "key-auth": { "key": key } } }] }));
    let remote = config(json!({ "consumers": [{ "username": name, "plugins": {} }] }));

    let expected = ev(
        ResourceType::Consumer,
        EventKind::Update {
            old_value: json!({ "plugins": {}, "username": name }),
            new_value: json!({ "plugins": { "key-auth": { "key": key } }, "username": name }),
            diff: Some(vec![adc_sdk::ValueDiff::New {
                path: vec![adc_sdk::PathSegment::Key("plugins".into()), adc_sdk::PathSegment::Key("key-auth".into())],
                rhs: json!({ "key": key }),
            }]),
        },
        name,
        name,
    );

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn update_resource_update_plugin_with_default_value() {
    let name = "alice";
    let old_key = "old-key";
    let new_key = "new-key";
    let local = config(json!({ "consumers": [{ "username": name, "plugins": { "key-auth": { "key": new_key } } }] }));
    let remote = config(
        json!({ "consumers": [{ "username": name, "plugins": { "key-auth": { "key": old_key, "added": "added" } } }] }),
    );
    let default_value = DefaultValue {
        core: HashMap::new(),
        plugins: HashMap::from([("key-auth".to_string(), json!({ "added": "added" }))]),
    };

    let expected = ev(
        ResourceType::Consumer,
        EventKind::Update {
            old_value: json!({ "plugins": { "key-auth": { "added": "added", "key": old_key } }, "username": name }),
            new_value: json!({ "plugins": { "key-auth": { "key": new_key, "added": "added" } }, "username": name }),
            diff: Some(vec![adc_sdk::ValueDiff::Edit {
                path: vec![
                    adc_sdk::PathSegment::Key("plugins".into()),
                    adc_sdk::PathSegment::Key("key-auth".into()),
                    adc_sdk::PathSegment::Key("key".into()),
                ],
                lhs: json!(old_key),
                rhs: json!(new_key),
            }]),
        },
        name,
        name,
    );

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![expected]);
}

#[test]
fn generates_hashed_resource_id() {
    let ssl_name = "demo-sni1,demo-sni2";
    let ssl = json!({
        "snis": ["demo-sni1", "demo-sni2"],
        "certificates": [{ "certificate": "cert", "key": "key" }],
    });
    let local = config(json!({ "ssls": [ssl] }));
    let remote = config(json!({}));

    let expected = ev(ResourceType::Ssl, EventKind::Create { new_value: ssl }, &generate_id(ssl_name), ssl_name);

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn updates_service_nested_route() {
    let service_name = "Test Service";
    let route_name = "Test Route";

    let local = config(json!({
        "services": [{
            "name": service_name,
            "routes": [{ "name": route_name, "uris": ["/test"], "plugins": { "test": { "testKey": "newValue" } } }],
        }]
    }));
    let remote = config(json!({
        "services": [{
            "id": generate_id(service_name),
            "name": service_name,
            "routes": [{
                "id": generate_id(&format!("{service_name}.{route_name}")),
                "name": route_name,
                "uris": ["/test"],
                "plugins": { "test": { "testKey": "oldValue" } },
            }],
        }]
    }));

    let mut expected = ev(
        ResourceType::Route,
        EventKind::Update {
            old_value: json!({ "name": route_name, "uris": ["/test"], "plugins": { "test": { "testKey": "oldValue" } } }),
            new_value: json!({ "name": route_name, "uris": ["/test"], "plugins": { "test": { "testKey": "newValue" } } }),
            diff: Some(vec![adc_sdk::ValueDiff::Edit {
                path: vec![
                    adc_sdk::PathSegment::Key("plugins".into()),
                    adc_sdk::PathSegment::Key("test".into()),
                    adc_sdk::PathSegment::Key("testKey".into()),
                ],
                lhs: json!("oldValue"),
                rhs: json!("newValue"),
            }]),
        },
        &generate_id(&format!("{service_name}.{route_name}")),
        route_name,
    );
    expected.parent_id = Some(generate_id(service_name));

    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![expected]);
}

#[test]
fn updates_service_and_its_nested_route() {
    let service_name = "Test Service";
    let service_id = generate_id(service_name);
    let route_name = "Test Route";
    let route_id = generate_id(&format!("{service_name}.{route_name}"));

    let local = config(json!({
        "services": [{
            "name": service_name,
            "path_prefix": "/test",
            "plugins": { "test": { "testKey": "serviceNewValue" } },
            "routes": [{ "name": route_name, "uris": ["/test"], "plugins": { "test": { "testKey": "newValue" } } }],
        }]
    }));
    let remote = config(json!({
        "services": [{
            "id": service_id,
            "name": service_name,
            "plugins": { "test": { "testKey": "serviceOldValue" } },
            "routes": [{
                "id": route_id,
                "name": route_name,
                "uris": ["/test"],
                "plugins": { "test": { "testKey": "oldValue" } },
            }],
        }]
    }));

    let mut route_ev = ev(
        ResourceType::Route,
        EventKind::Update {
            old_value: json!({ "name": route_name, "uris": ["/test"], "plugins": { "test": { "testKey": "oldValue" } } }),
            new_value: json!({ "name": route_name, "uris": ["/test"], "plugins": { "test": { "testKey": "newValue" } } }),
            diff: Some(vec![adc_sdk::ValueDiff::Edit {
                path: vec![
                    adc_sdk::PathSegment::Key("plugins".into()),
                    adc_sdk::PathSegment::Key("test".into()),
                    adc_sdk::PathSegment::Key("testKey".into()),
                ],
                lhs: json!("oldValue"),
                rhs: json!("newValue"),
            }]),
        },
        &route_id,
        route_name,
    );
    route_ev.parent_id = Some(service_id.clone());

    let service_ev = ev(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({ "name": service_name, "plugins": { "test": { "testKey": "serviceOldValue" } } }),
            new_value: json!({ "name": service_name, "path_prefix": "/test", "plugins": { "test": { "testKey": "serviceNewValue" } } }),
            diff: Some(vec![
                adc_sdk::ValueDiff::Edit {
                    path: vec![
                        adc_sdk::PathSegment::Key("plugins".into()),
                        adc_sdk::PathSegment::Key("test".into()),
                        adc_sdk::PathSegment::Key("testKey".into()),
                    ],
                    lhs: json!("serviceOldValue"),
                    rhs: json!("serviceNewValue"),
                },
                adc_sdk::ValueDiff::New { path: vec![adc_sdk::PathSegment::Key("path_prefix".into())], rhs: json!("/test") },
            ]),
        },
        &service_id,
        service_name,
    );

    // ROUTE.UPDATE (10) sorts before SERVICE.UPDATE (12).
    assert_eq!(DifferV4::diff(&local, &remote, None, None), vec![route_ev, service_ev]);
}

#[test]
fn keeps_plugins_when_plugins_not_changed() {
    let service_name = "Test Service";
    let service_id = generate_id(service_name);

    let local = config(json!({
        "services": [{
            "name": service_name,
            "path_prefix": "/test",
            "plugins": { "test": { "testKey": "testValue" } },
        }]
    }));
    let remote = config(json!({
        "services": [{
            "id": service_id,
            "name": service_name,
            "plugins": { "test": { "testKey": "testValue", "added": "added" } },
        }]
    }));
    let default_value = DefaultValue {
        core: HashMap::new(),
        plugins: HashMap::from([("test".to_string(), json!({ "added": "added" }))]),
    };

    let expected = ev(
        ResourceType::Service,
        EventKind::Update {
            old_value: json!({
                "name": service_name,
                "plugins": { "test": { "testKey": "testValue", "added": "added" } },
            }),
            new_value: json!({
                "name": service_name,
                "path_prefix": "/test",
                "plugins": { "test": { "testKey": "testValue", "added": "added" } },
            }),
            diff: Some(vec![adc_sdk::ValueDiff::New { path: vec![adc_sdk::PathSegment::Key("path_prefix".into())], rhs: json!("/test") }]),
        },
        &service_id,
        service_name,
    );

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![expected]);
}

#[test]
fn selectively_merges_objects_in_default_values() {
    let service_id = generate_id("Test Service");
    let local = config(json!({ "services": [{ "name": "Test Service", "test1": {} }] }));
    let remote = config(json!({
        "services": [{
            "id": service_id,
            "name": "Test Service",
            "test": "test",
            "test1": { "test2": "test2" },
        }]
    }));
    let default_value = DefaultValue {
        core: HashMap::from([(
            ResourceType::Service,
            json!({ "test": "test", "test1": { "test2": "test2", "test3": { "test4": "test4" } } }),
        )]),
        plugins: HashMap::new(),
    };

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![]);
}

#[test]
fn merges_array_nested_object_defaults_correctly() {
    let service_name = "Test Service";
    let new_node = json!({ "host": "0.0.0.0", "port": 443, "weight": 1 });
    let mut old_node = new_node.clone();
    old_node["priority"] = json!(0);

    let local = config(json!({ "services": [{ "name": service_name, "upstream": { "nodes": [new_node] } }] }));
    let remote = config(json!({
        "services": [{ "id": generate_id(service_name), "name": service_name, "upstream": { "nodes": [old_node] } }]
    }));
    let default_value = DefaultValue {
        core: HashMap::from([(
            ResourceType::Service,
            json!({ "upstream": { "nodes": [{ "priority": 0 }] } }),
        )]),
        plugins: HashMap::new(),
    };

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![]);
}

#[test]
fn route_and_stream_route_ids_generated_correctly() {
    let new_services = json!([
        { "name": "HTTP", "routes": [{ "name": "HTTP 1", "uris": ["/1"] }] },
        { "name": "Stream", "stream_routes": [{ "name": "Stream 1", "server_port": 5432 }] },
    ]);
    let mut old_services = new_services.clone();
    old_services[0]["id"] = json!(generate_id("HTTP"));
    old_services[0]["routes"][0]["id"] = json!(generate_id("HTTP.HTTP 1"));
    old_services[1]["id"] = json!(generate_id("Stream"));
    old_services[1]["stream_routes"][0]["id"] = json!(generate_id("Stream.Stream 1"));

    let local = config(json!({ "services": new_services }));
    let remote = config(json!({ "services": old_services }));

    assert_eq!(DifferV4::diff(&local, &remote, Some(&DefaultValue::default()), None), vec![]);
}

#[test]
fn boolean_defaults_merged_correctly() {
    let service = json!({ "name": "HTTP", "path_prefix": "/test", "strip_path_prefix": false });
    let mut old_service = service.clone();
    old_service["id"] = json!(generate_id("HTTP"));
    old_service["strip_path_prefix"] = json!(true);

    let local = config(json!({ "services": [service.clone()] }));
    let remote = config(json!({ "services": [old_service] }));
    let default_value = DefaultValue {
        core: HashMap::from([(ResourceType::Service, json!({ "strip_path_prefix": true }))]),
        plugins: HashMap::new(),
    };

    let mut expected_old = service.clone();
    expected_old["strip_path_prefix"] = json!(true);

    let expected = ev(
        ResourceType::Service,
        EventKind::Update {
            old_value: expected_old,
            new_value: service,
            diff: Some(vec![adc_sdk::ValueDiff::Edit {
                path: vec![adc_sdk::PathSegment::Key("strip_path_prefix".into())],
                lhs: json!(true),
                rhs: json!(false),
            }]),
        },
        &generate_id("HTTP"),
        "HTTP",
    );

    assert_eq!(DifferV4::diff(&local, &remote, Some(&default_value), None), vec![expected]);
}
