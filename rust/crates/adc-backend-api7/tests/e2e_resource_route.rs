//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_resource_route -- --ignored --test-threads=1`.

use adc_sdk::ResourceType;
use semver::Version;
use serde_json::json;

mod common;
use common::{
    assert_matches_object, create_event, delete_event, dump_configuration, server_version,
    sync_events,
};

#[tokio::test]
#[ignore]
async fn route_timeout_round_trips_through_sync_and_dump() {
    let backend = common::backend().await;
    let service_name = "test";
    let service = json!({
        "name": service_name,
        "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
        "path_prefix": "/test",
        "strip_path_prefix": true,
    });
    let route1_name = "route1";
    let route1 = json!({ "name": route1_name, "uris": ["/route1"], "timeout": { "connect": 111, "send": 222, "read": 333 } });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service_name, service.clone(), None),
            create_event(
                ResourceType::Route,
                route1_name,
                route1.clone(),
                Some(service_name),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service);
    let routes = services[0].routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.len(), 1);
    assert_matches_object(&serde_json::to_value(&routes[0]).unwrap(), &route1);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Service, service_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}

/// Regression guard for the whole-number-only timeout bug fixed by
/// `typing::serialize_timeout`: a route/upstream timeout that's genuinely
/// fractional (not just `111.0` with a spurious decimal point) must still
/// round-trip unchanged now that the dashboard accepts it.
#[tokio::test]
#[ignore]
async fn fractional_timeout_and_retry_timeout_round_trip_through_sync_and_dump() {
    let backend = common::backend().await;
    let service_name = "test-fractional-timeout";
    let service = json!({
        "name": service_name,
        "upstream": {
            "scheme": "https",
            "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }],
            "retry_timeout": 60.5,
        },
        "path_prefix": "/test",
        "strip_path_prefix": true,
    });
    let route1_name = "route1";
    let route1 = json!({
        "name": route1_name,
        "uris": ["/route1"],
        "timeout": { "connect": 60.5, "send": 60.5, "read": 60.5 },
    });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service_name, service.clone(), None),
            create_event(
                ResourceType::Route,
                route1_name,
                route1.clone(),
                Some(service_name),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service);
    let routes = services[0].routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.len(), 1);
    assert_matches_object(&serde_json::to_value(&routes[0]).unwrap(), &route1);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Service, service_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}

#[tokio::test]
#[ignore]
async fn route_vars_round_trips_through_sync_and_dump() {
    if server_version() < Version::new(3, 2, 16) {
        eprintln!("skipping: only applies from 3.2.16");
        return;
    }
    let backend = common::backend().await;
    let service_name = "test";
    let service = json!({
        "name": service_name,
        "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
        "path_prefix": "/test",
        "strip_path_prefix": true,
    });
    let route1_name = "route1";
    let route1 = json!({ "name": route1_name, "uris": ["/route1"], "vars": [["remote_addr", "==", "1.1.1.1"]] });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service_name, service.clone(), None),
            create_event(
                ResourceType::Route,
                route1_name,
                route1.clone(),
                Some(service_name),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service);
    let routes = services[0].routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.len(), 1);
    assert_matches_object(&serde_json::to_value(&routes[0]).unwrap(), &route1);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Service, service_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}
