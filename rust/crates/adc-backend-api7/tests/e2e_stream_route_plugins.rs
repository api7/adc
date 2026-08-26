//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_stream_route_plugins -- --ignored --test-threads=1`.

use adc_sdk::ResourceType;
use serde_json::json;

mod common;
use common::{create_event, delete_event, dump_configuration, sync_events, update_event};

/// Regression: the stream route read-direction conversion used to drop
/// `plugins`, so a dumped stream route always came back without plugins,
/// and the differ could never detect a plugin removal (local empty ===
/// remote empty), leaving stale plugins on the gateway.
#[tokio::test]
#[ignore]
async fn stream_route_plugin_round_trip_and_removal() {
    let backend = common::backend().await;
    let service_name = "stream-service";
    let service = json!({ "name": service_name, "upstream": { "scheme": "tcp", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] } });
    let stream_route_name = "stream-route";
    let plugins = json!({ "ip-restriction": { "whitelist": ["127.0.0.0/24"] } });
    let stream_route = json!({ "name": stream_route_name, "plugins": plugins });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service_name, service, None),
            create_event(
                ResourceType::StreamRoute,
                stream_route_name,
                stream_route.clone(),
                Some(service_name),
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let svc = dump
        .services
        .as_ref()
        .unwrap()
        .iter()
        .find(|s| s.name == service_name)
        .unwrap();
    let stream_routes = svc.routes.as_ref().unwrap().stream().unwrap();
    assert_eq!(stream_routes.len(), 1);
    assert_eq!(
        serde_json::to_value(&stream_routes[0].plugins).unwrap(),
        plugins
    );

    let mut cleared = stream_route.clone();
    cleared["plugins"] = json!({});
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::StreamRoute,
            stream_route_name,
            cleared,
            Some(service_name),
        )],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let svc = dump
        .services
        .as_ref()
        .unwrap()
        .iter()
        .find(|s| s.name == service_name)
        .unwrap();
    let stream_routes = svc.routes.as_ref().unwrap().stream().unwrap();
    assert_eq!(stream_routes.len(), 1);
    let plugins_after = stream_routes[0].plugins.clone().unwrap_or_default();
    assert!(
        plugins_after.is_empty(),
        "expected no plugins, got {plugins_after:?}"
    );

    sync_events(
        &backend,
        vec![
            delete_event(
                ResourceType::StreamRoute,
                stream_route_name,
                Some(service_name),
            ),
            delete_event(ResourceType::Service, service_name, None),
        ],
    )
    .await
    .unwrap();
}
