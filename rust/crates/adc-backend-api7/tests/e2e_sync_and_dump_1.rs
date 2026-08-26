//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_sync_and_dump_1 -- --ignored --test-threads=1`.

use adc_sdk::ResourceType;
use serde_json::json;

mod common;
use common::{
    assert_matches_object, create_event, delete_event, dump_configuration, sync_events, update_event,
};

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_single_service() {
    let backend = common::backend().await;
    let upstream = json!({ "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] });
    let service1_name = "service1";
    let mut service1 = json!({ "name": service1_name, "upstream": upstream });
    let service2_name = "service2";
    let service2 = json!({ "name": service2_name, "upstream": upstream });

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service1_name, service1.clone(), None),
            create_event(ResourceType::Service, service2_name, service2.clone(), None),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let mut services = dump.services.unwrap();
    services.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(services.len(), 2);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service1);
    assert_matches_object(&serde_json::to_value(&services[1]).unwrap(), &service2);

    service1["description"] = json!("desc");
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::Service,
            service1_name,
            service1.clone(),
            None,
        )],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let mut services = dump.services.unwrap();
    services.sort_by(|a, b| a.name.cmp(&b.name));
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service1);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Service, service1_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service2);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Service, service2_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_a_service_with_routes() {
    let backend = common::backend().await;
    let service_name = "test";
    let service = json!({
        "name": service_name,
        "upstream": { "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
        "path_prefix": "/test",
        "strip_path_prefix": true,
    });
    let route1_name = "route1";
    let route1 = json!({ "name": route1_name, "uris": ["/route1", "/route1-2"], "priority": 100 });
    let route2_name = "route2";
    let route2 = json!({ "name": route2_name, "uris": ["/route2", "/route2-2"], "plugins": { "key-auth": {} } });

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
            create_event(
                ResourceType::Route,
                route2_name,
                route2.clone(),
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
    let mut routes = services[0]
        .routes
        .as_ref()
        .unwrap()
        .http()
        .unwrap()
        .to_vec();
    routes.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(routes.len(), 2);
    assert_matches_object(&serde_json::to_value(&routes[0]).unwrap(), &route1);
    assert_matches_object(&serde_json::to_value(&routes[1]).unwrap(), &route2);

    sync_events(
        &backend,
        vec![delete_event(
            ResourceType::Route,
            route1_name,
            Some(service_name),
        )],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    let routes = services[0].routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.len(), 1);
    assert_matches_object(&serde_json::to_value(&routes[0]).unwrap(), &route2);

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
async fn syncs_and_dumps_a_service_with_stream_routes() {
    let backend = common::backend().await;
    let service_name = "test";
    let service = json!({ "name": service_name, "upstream": { "scheme": "tcp", "nodes": [{ "host": "1.1.1.1", "port": 853, "weight": 100 }] } });
    let route1_name = "sroute1";
    let route1 = json!({ "name": route1_name, "server_port": 5432 });
    let route2_name = "sroute2";
    let route2 = json!({ "name": route2_name, "server_port": 3306 });
    let mut service_for_sync = service.clone();
    service_for_sync["stream_routes"] = json!([]);

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Service, service_name, service_for_sync, None),
            create_event(
                ResourceType::StreamRoute,
                route1_name,
                route1.clone(),
                Some(service_name),
            ),
            create_event(
                ResourceType::StreamRoute,
                route2_name,
                route2.clone(),
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
    let mut stream_routes = services[0]
        .routes
        .as_ref()
        .unwrap()
        .stream()
        .unwrap()
        .to_vec();
    stream_routes.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(stream_routes.len(), 2);
    assert_matches_object(&serde_json::to_value(&stream_routes[0]).unwrap(), &route2);
    assert_matches_object(&serde_json::to_value(&stream_routes[1]).unwrap(), &route1);

    sync_events(
        &backend,
        vec![delete_event(
            ResourceType::StreamRoute,
            route1_name,
            Some(service_name),
        )],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service);
    let stream_routes = services[0].routes.as_ref().unwrap().stream().unwrap();
    assert_eq!(stream_routes.len(), 1);
    assert_matches_object(&serde_json::to_value(&stream_routes[0]).unwrap(), &route2);

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
async fn syncs_and_dumps_global_rules() {
    let backend = common::backend().await;
    let plugin1_name = "prometheus";
    let mut plugin1 = json!({ "prefer_name": true });
    let plugin2_name = "file-logger";
    let plugin2 = json!({ "path": "logs/file.log" });

    sync_events(
        &backend,
        vec![
            create_event(
                ResourceType::GlobalRule,
                plugin1_name,
                plugin1.clone(),
                None,
            ),
            create_event(
                ResourceType::GlobalRule,
                plugin2_name,
                plugin2.clone(),
                None,
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let global_rules = dump.global_rules.unwrap();
    assert_eq!(global_rules.len(), 2);
    assert_matches_object(&global_rules[plugin1_name], &plugin1);
    assert_matches_object(&global_rules[plugin2_name], &plugin2);

    plugin1["test"] = json!("test");
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::GlobalRule,
            plugin1_name,
            plugin1.clone(),
            None,
        )],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    assert_matches_object(&dump.global_rules.unwrap()[plugin1_name], &plugin1);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::GlobalRule, plugin1_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let global_rules = dump.global_rules.unwrap();
    assert_eq!(global_rules.len(), 1);
    assert!(!global_rules.contains_key(plugin1_name));
    assert_matches_object(&global_rules[plugin2_name], &plugin2);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::GlobalRule, plugin2_name, None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.global_rules.is_none_or(|g| g.is_empty()));
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_plugin_metadata() {
    let backend = common::backend().await;
    let plugin1_name = "http-logger";
    let mut plugin1 = json!({ "log_format": { "test": "test", "test1": "test1" } });
    let plugin2_name = "tcp-logger";
    let plugin2 = json!({ "log_format": { "test": "test", "test1": "test1" } });

    sync_events(
        &backend,
        vec![
            create_event(
                ResourceType::PluginMetadata,
                plugin1_name,
                plugin1.clone(),
                None,
            ),
            create_event(
                ResourceType::PluginMetadata,
                plugin2_name,
                plugin2.clone(),
                None,
            ),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let plugin_metadata = dump.plugin_metadata.unwrap();
    assert_eq!(plugin_metadata.len(), 2);
    assert_matches_object(&plugin_metadata[plugin1_name], &plugin1);
    assert_matches_object(&plugin_metadata[plugin2_name], &plugin2);

    plugin1["test"] = json!("test");
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::PluginMetadata,
            plugin1_name,
            plugin1.clone(),
            None,
        )],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    assert_matches_object(&dump.plugin_metadata.unwrap()[plugin1_name], &plugin1);

    sync_events(
        &backend,
        vec![delete_event(
            ResourceType::PluginMetadata,
            plugin1_name,
            None,
        )],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let plugin_metadata = dump.plugin_metadata.unwrap();
    assert_eq!(plugin_metadata.len(), 1);
    assert!(!plugin_metadata.contains_key(plugin1_name));
    assert_matches_object(&plugin_metadata[plugin2_name], &plugin2);

    sync_events(
        &backend,
        vec![delete_event(
            ResourceType::PluginMetadata,
            plugin2_name,
            None,
        )],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.plugin_metadata.is_none_or(|p| p.is_empty()));
}
