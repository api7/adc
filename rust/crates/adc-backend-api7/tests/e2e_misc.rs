//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_misc -- --ignored --test-threads=1`.

use adc_sdk::ResourceType;
use serde_json::json;

mod common;
use common::{
    assert_matches_object, create_event, delete_event, dump_configuration,
    override_event_resource_id, sync_events, sync_events_with_opts,
};

#[tokio::test]
#[ignore]
async fn syncs_resources_whose_name_and_description_exceed_256_bytes() {
    let backend = common::backend().await;
    let route_name = "0".repeat(64 * 1024);
    let service_name = "0".repeat(64 * 1024);
    let route = json!({ "name": route_name, "uris": ["/test"] });
    let service = json!({
        "name": service_name,
        "description": "0".repeat(64 * 1024),
        "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
    });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, &service_name, service.clone(), None),
            create_event(
                ResourceType::Route,
                &route_name,
                route.clone(),
                Some(&service_name),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service);
    assert_matches_object(
        &serde_json::to_value(&services[0].routes.as_ref().unwrap().http().unwrap()[0]).unwrap(),
        &route,
    );

    sync_events(
        &backend,
        vec![
            delete_event(ResourceType::Route, &route_name, Some(&service_name)),
            delete_event(ResourceType::Service, &service_name, None),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}

#[tokio::test]
#[ignore]
async fn syncs_resources_with_a_user_supplied_custom_id() {
    let backend = common::backend().await;
    let route_name = "Test Route";
    let service_name = "Test Service";
    let route = json!({ "id": "custom-route", "name": route_name, "uris": ["/test"] });
    let service = json!({
        "id": "custom-service",
        "name": service_name,
        "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
    });

    sync_events(
        &backend,
        vec![
            override_event_resource_id(
                create_event(ResourceType::Service, service_name, service.clone(), None),
                "custom-service",
                None,
            ),
            override_event_resource_id(
                create_event(
                    ResourceType::Route,
                    route_name,
                    route.clone(),
                    Some(service_name),
                ),
                "custom-route",
                Some("custom-service"),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service);
    assert_matches_object(
        &serde_json::to_value(&services[0].routes.as_ref().unwrap().http().unwrap()[0]).unwrap(),
        &route,
    );

    sync_events(
        &backend,
        vec![
            override_event_resource_id(
                delete_event(ResourceType::Route, route_name, Some(service_name)),
                "custom-route",
                Some("custom-service"),
            ),
            override_event_resource_id(
                delete_event(ResourceType::Service, service_name, None),
                "custom-service",
                None,
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}

/// `service2`'s `path_prefix` is a number, not a string, so its event fails
/// to deserialize into `adc_sdk::resources::Service` client-side — a
/// `BackendError::Serialization` — before any request reaches the server at
/// all, rather than the dashboard's own schema validation rejecting it.
#[tokio::test]
#[ignore]
async fn sync_options_exit_on_failure() {
    let backend = common::backend().await;
    let upstream = json!({ "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] });
    let service1_name = "service1";
    let service1 = json!({ "name": service1_name, "upstream": upstream });
    let service2_name = "service2";
    let service2 = json!({ "name": service2_name, "path_prefix": 12345, "upstream": upstream });

    let error = sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service1_name, service1.clone(), None),
            create_event(ResourceType::Service, service2_name, service2.clone(), None),
        ],
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, adc_sdk::BackendError::Serialization(_)),
        "{error:?}"
    );

    // No cleanup needed between the two sync attempts: syncing the same
    // service again is a `PUT` on the same deterministic id, an idempotent
    // update rather than a conflicting create.
    let results = sync_events_with_opts(
        &backend,
        vec![
            create_event(ResourceType::Service, service1_name, service1, None),
            create_event(ResourceType::Service, service2_name, service2, None),
        ],
        adc_sdk::BackendSyncOptions {
            exit_on_failure: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(results.iter().filter(|r| r.success).count(), 1);
    let failed = results
        .iter()
        .find(|r| !r.success)
        .expect("one result should have failed");
    assert!(
        matches!(
            failed.error.as_ref().unwrap(),
            adc_sdk::BackendError::Serialization(_)
        ),
        "{:?}",
        failed.error
    );

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Service, service1_name, None)],
    )
    .await
    .unwrap();
}
