//! Ported from `libs/backend-apisix/e2e/sync-and-dump-1.e2e-spec.ts`. Real
//! network calls against a live APISIX — see `e2e_apisix.rs`'s module doc
//! for how to bring one up and run this file.
//!
//! Each TS `describe` block (a sequence of dependent `it`s sharing mutable
//! state) becomes one `#[tokio::test]` function running the same
//! create/dump/update/dump/delete/dump sequence linearly. Assertions that
//! depended on etcd's incidental list-return order (`services[0]` /
//! `services[1]`) are rewritten to find by id instead — that ordering was
//! never a property of *our* code, just an accident of how etcd happens to
//! range-scan keys, so asserting on it would be testing the wrong thing.

use std::time::Duration;

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::resources::Configuration;
use adc_sdk::utils::generate_id;
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, Event, EventKind, ResourceType};
use serde_json::json;

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn backend() -> ApisixBackend {
    let client = HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap();
    ApisixBackend::new(client)
}

fn apisix_version() -> semver::Version {
    std::env::var("BACKEND_APISIX_VERSION").ok().and_then(|v| semver::Version::parse(&v).ok()).unwrap_or(semver::Version::new(999, 999, 999))
}

/// Mirrors `createEvent`'s `resourceId` derivation in
/// `libs/backend-apisix/e2e/support/utils.ts`: APISIX keys consumers,
/// global rules and plugin metadata by their literal name (not a content
/// hash), everything else by `generate_id(name)` — or, when nested under a
/// parent, `generate_id("parent.name")`.
fn resource_id(rt: ResourceType, name: &str, parent_name: Option<&str>) -> String {
    match rt {
        ResourceType::Consumer | ResourceType::GlobalRule | ResourceType::PluginMetadata => name.to_string(),
        _ => match parent_name {
            Some(parent) => generate_id(&format!("{parent}.{name}")),
            None => generate_id(name),
        },
    }
}

fn create(rt: ResourceType, name: &str, new_value: serde_json::Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, resource_id(rt, name, None), name)
}

fn create_child(rt: ResourceType, name: &str, new_value: serde_json::Value, parent_name: &str) -> Event {
    let mut event = Event::new(rt, EventKind::Create { new_value }, resource_id(rt, name, Some(parent_name)), name);
    event.parent_id = Some(resource_id(ResourceType::Service, parent_name, None));
    event
}

/// Computes a real diff (via `adc_sdk::diff_value`, the same function the
/// differ itself uses) rather than leaving it `None` — for `SERVICE`
/// updates specifically, the operator inspects `diff` to decide whether the
/// service's own admin-API resource needs touching at all versus only its
/// upstream (see `operator.rs`'s module doc comment); a `None`/empty diff
/// reads as "nothing outside upstream changed" and skips the main request
/// entirely, which would silently no-op a hand-built event with no diff.
fn update(rt: ResourceType, name: &str, old_value: serde_json::Value, new_value: serde_json::Value) -> Event {
    let diff = adc_sdk::diff_value(&old_value, &new_value);
    Event::new(rt, EventKind::Update { old_value, new_value, diff }, resource_id(rt, name, None), name)
}

fn delete(rt: ResourceType, name: &str) -> Event {
    Event::new(rt, EventKind::Delete { old_value: json!({}) }, resource_id(rt, name, None), name)
}

fn delete_child(rt: ResourceType, name: &str, parent_name: &str) -> Event {
    let mut event = Event::new(rt, EventKind::Delete { old_value: json!({}) }, resource_id(rt, name, Some(parent_name)), name);
    event.parent_id = Some(resource_id(ResourceType::Service, parent_name, None));
    event
}

async fn sync_ok(backend: &ApisixBackend, events: Vec<Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?} {}: {:?}", result.event.resource_type, result.event.resource_id, result.error);
    }
}

async fn dump(backend: &ApisixBackend) -> Configuration {
    backend.dump().await.unwrap()
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_services_lifecycle() {
    let service1_name = "service1";
    let service2_name = "service2";
    let backend = backend();
    let upstream = json!({ "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] });

    sync_ok(
        &backend,
        vec![
            create(ResourceType::Service, service1_name, json!({ "name": service1_name, "upstream": upstream, "hosts": ["example1.com", "example2.com"] })),
            create(ResourceType::Service, service2_name, json!({ "name": service2_name, "upstream": upstream })),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let services = config.services.unwrap();
    assert_eq!(services.len(), 2);
    let service1 = services.iter().find(|s| s.name == service1_name).expect("service1 missing");
    assert_eq!(service1.hosts.as_deref(), Some(&["example1.com".to_string(), "example2.com".to_string()][..]));
    assert!(services.iter().any(|s| s.name == service2_name));

    sync_ok(
        &backend,
        vec![update(
            ResourceType::Service,
            service1_name,
            json!({ "name": service1_name, "upstream": upstream, "hosts": ["example1.com", "example2.com"] }),
            json!({ "name": service1_name, "upstream": upstream, "hosts": ["example1.com", "example2.com"], "description": "desc" }),
        )],
    )
    .await;

    let config = dump(&backend).await;
    let service1 = config.services.unwrap().into_iter().find(|s| s.name == service1_name).expect("service1 missing");
    assert_eq!(service1.description.as_deref(), Some("desc"));

    sync_ok(&backend, vec![delete(ResourceType::Service, service1_name)]).await;
    let config = dump(&backend).await;
    let services = config.services.unwrap();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].name, service2_name);

    sync_ok(&backend, vec![delete(ResourceType::Service, service2_name)]).await;
    let config = dump(&backend).await;
    assert!(config.services.is_none() || config.services.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_service_with_routes_lifecycle() {
    let service_name = "test";
    let route1_name = "route1";
    let route2_name = "route2";
    let backend = backend();
    let upstream = json!({ "scheme": "https", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] });

    sync_ok(
        &backend,
        vec![
            create(ResourceType::Service, service_name, json!({ "name": service_name, "upstream": upstream })),
            create_child(ResourceType::Route, route1_name, json!({ "name": route1_name, "uris": ["/route1"] }), service_name),
            create_child(ResourceType::Route, route2_name, json!({ "name": route2_name, "uris": ["/route2"], "plugins": { "key-auth": {} } }), service_name),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let services = config.services.unwrap();
    assert_eq!(services.len(), 1);
    let routes = services[0].routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.len(), 2);
    assert!(routes.iter().any(|r| r.name == route1_name && r.uris == vec!["/route1".to_string()]));
    assert!(routes.iter().any(|r| r.name == route2_name && r.plugins.is_some()));

    sync_ok(&backend, vec![delete_child(ResourceType::Route, route1_name, service_name)]).await;
    let config = dump(&backend).await;
    let services = config.services.unwrap();
    let routes = services[0].routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].name, route2_name);

    sync_ok(&backend, vec![delete_child(ResourceType::Route, route2_name, service_name)]).await;
    // See the TS spec's comment: APISIX checks referential integrity
    // against its own etcd-watch-derived in-memory cache, which lags
    // slightly behind the admin API write that created the delete event —
    // deleting a service right after its last route needs a short pause or
    // the delete is flaky.
    tokio::time::sleep(Duration::from_millis(200)).await;
    sync_ok(&backend, vec![delete(ResourceType::Service, service_name)]).await;

    let config = dump(&backend).await;
    assert!(config.services.is_none() || config.services.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_service_with_stream_route_lifecycle() {
    if apisix_version() < semver::Version::new(3, 7, 0) {
        eprintln!("skipping: stream routes require apisix >= 3.7.0");
        return;
    }

    let service_name = "test";
    let stream_route_name = "postgres";
    let backend = backend();
    let upstream = json!({ "scheme": "tcp", "nodes": [{ "host": "1.1.1.1", "port": 5432, "weight": 100 }] });

    sync_ok(
        &backend,
        vec![
            create(ResourceType::Service, service_name, json!({ "name": service_name, "upstream": upstream })),
            create_child(ResourceType::StreamRoute, stream_route_name, json!({ "name": stream_route_name, "server_port": 54320 }), service_name),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let services = config.services.unwrap();
    assert_eq!(services.len(), 1);
    let stream_routes = services[0].routes.as_ref().unwrap().stream().unwrap();
    assert_eq!(stream_routes.len(), 1);
    assert_eq!(stream_routes[0].server_port, Some(54320));
    // Below 3.8.0 the `__ADC_NAME` label is never written (see
    // `transformer.rs`), so recovery falls back to the route's own id —
    // which happens to equal `stream_route_name` too in this fixture, so
    // the assertion holds either way (matches the TS suite's
    // `Dump (<3.8.0)` and `Dump (>=3.8.0)` cases both).
    assert_eq!(stream_routes[0].name, stream_route_name);

    sync_ok(&backend, vec![delete_child(ResourceType::StreamRoute, stream_route_name, service_name)]).await;
    let config = dump(&backend).await;
    let services = config.services.unwrap();
    assert!(services[0].routes.is_none());

    tokio::time::sleep(Duration::from_millis(200)).await;
    sync_ok(&backend, vec![delete(ResourceType::Service, service_name)]).await;
    let config = dump(&backend).await;
    assert!(config.services.is_none() || config.services.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_consumers_lifecycle() {
    let consumer1_name = "consumer1";
    let consumer2_name = "consumer2";
    let backend = backend();

    sync_ok(
        &backend,
        vec![
            create(ResourceType::Consumer, consumer1_name, json!({ "username": consumer1_name, "plugins": { "key-auth": { "key": consumer1_name } } })),
            create(ResourceType::Consumer, consumer2_name, json!({ "username": consumer2_name, "plugins": { "key-auth": { "key": consumer2_name } } })),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let consumers = config.consumers.unwrap();
    assert_eq!(consumers.len(), 2);
    assert!(consumers.iter().any(|c| c.username == consumer1_name));
    assert!(consumers.iter().any(|c| c.username == consumer2_name));

    sync_ok(
        &backend,
        vec![update(
            ResourceType::Consumer,
            consumer1_name,
            json!({ "username": consumer1_name, "plugins": { "key-auth": { "key": consumer1_name } } }),
            json!({ "username": consumer1_name, "plugins": { "key-auth": { "key": consumer1_name } }, "description": "desc" }),
        )],
    )
    .await;
    let config = dump(&backend).await;
    let consumer1 = config.consumers.unwrap().into_iter().find(|c| c.username == consumer1_name).unwrap();
    assert_eq!(consumer1.description.as_deref(), Some("desc"));

    sync_ok(&backend, vec![delete(ResourceType::Consumer, consumer1_name)]).await;
    let config = dump(&backend).await;
    let consumers = config.consumers.unwrap();
    assert_eq!(consumers.len(), 1);
    assert_eq!(consumers[0].username, consumer2_name);

    sync_ok(&backend, vec![delete(ResourceType::Consumer, consumer2_name)]).await;
    let config = dump(&backend).await;
    assert!(config.consumers.is_none() || config.consumers.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_ssls_lifecycle() {
    let backend = backend();
    let cert = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../libs/backend-apisix/e2e/assets/test-ssl.cer"),
    )
    .unwrap();
    let key = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../libs/backend-apisix/e2e/assets/test-ssl.key"),
    )
    .unwrap();

    let ssl1_snis = ["ssl1-1.com", "ssl1-2.com"];
    let ssl2_snis = ["ssl2-1.com", "ssl2-2.com"];
    let ssl1_name = ssl1_snis.join(",");
    let ssl2_name = ssl2_snis.join(",");

    sync_ok(
        &backend,
        vec![
            create(ResourceType::Ssl, &ssl1_name, json!({ "snis": ssl1_snis, "certificates": [{ "certificate": cert, "key": key }] })),
            create(ResourceType::Ssl, &ssl2_name, json!({ "snis": ssl2_snis, "certificates": [{ "certificate": cert, "key": key }] })),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let ssls = config.ssls.unwrap();
    assert_eq!(ssls.len(), 2);
    assert!(ssls.iter().any(|s| s.snis == ssl1_snis));
    assert!(ssls.iter().any(|s| s.snis == ssl2_snis));

    sync_ok(
        &backend,
        vec![update(
            ResourceType::Ssl,
            &ssl1_name,
            json!({ "snis": ssl1_snis, "certificates": [{ "certificate": cert, "key": key }] }),
            json!({ "snis": ssl1_snis, "certificates": [{ "certificate": cert, "key": key }], "labels": { "test": "test" } }),
        )],
    )
    .await;
    let config = dump(&backend).await;
    let ssl1 = config.ssls.unwrap().into_iter().find(|s| s.snis == ssl1_snis).unwrap();
    assert_eq!(ssl1.labels.unwrap().get("test"), Some(&adc_sdk::resources::LabelValue::Single("test".to_string())));

    sync_ok(&backend, vec![delete(ResourceType::Ssl, &ssl1_name)]).await;
    let config = dump(&backend).await;
    let ssls = config.ssls.unwrap();
    assert_eq!(ssls.len(), 1);
    assert_eq!(ssls[0].snis, ssl2_snis);

    sync_ok(&backend, vec![delete(ResourceType::Ssl, &ssl2_name)]).await;
    let config = dump(&backend).await;
    assert!(config.ssls.is_none() || config.ssls.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_global_rules_lifecycle() {
    let rule1_name = "prometheus";
    let rule2_name = "file-logger";
    let backend = backend();

    sync_ok(
        &backend,
        vec![
            create(ResourceType::GlobalRule, rule1_name, json!({ "prefer_name": true })),
            create(ResourceType::GlobalRule, rule2_name, json!({ "path": "logs/file.log" })),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let rules = config.global_rules.unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules.get(rule1_name).and_then(|v| v.get("prefer_name")), Some(&json!(true)));

    sync_ok(
        &backend,
        vec![update(ResourceType::GlobalRule, rule1_name, json!({ "prefer_name": true }), json!({ "prefer_name": true, "test": "test" }))],
    )
    .await;
    let config = dump(&backend).await;
    assert_eq!(config.global_rules.unwrap().get(rule1_name).and_then(|v| v.get("test")), Some(&json!("test")));

    sync_ok(&backend, vec![delete(ResourceType::GlobalRule, rule1_name)]).await;
    let config = dump(&backend).await;
    let rules = config.global_rules.unwrap();
    assert_eq!(rules.len(), 1);
    assert!(rules.contains_key(rule2_name));

    sync_ok(&backend, vec![delete(ResourceType::GlobalRule, rule2_name)]).await;
    let config = dump(&backend).await;
    assert!(config.global_rules.is_none() || config.global_rules.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_plugin_metadata_lifecycle() {
    let metadata1_name = "http-logger";
    let metadata2_name = "tcp-logger";
    let backend = backend();

    sync_ok(
        &backend,
        vec![
            create(ResourceType::PluginMetadata, metadata1_name, json!({ "log_format": { "test": "test", "test1": "test1" } })),
            create(ResourceType::PluginMetadata, metadata2_name, json!({ "log_format": { "test": "test", "test1": "test1" } })),
        ],
    )
    .await;

    let config = dump(&backend).await;
    let metadata = config.plugin_metadata.unwrap();
    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata.get(metadata1_name).and_then(|v| v.get("log_format")).and_then(|v| v.get("test")), Some(&json!("test")));

    sync_ok(
        &backend,
        vec![update(
            ResourceType::PluginMetadata,
            metadata1_name,
            json!({ "log_format": { "test": "test", "test1": "test1" } }),
            json!({ "log_format": { "test": "test", "test1": "test1" }, "test": { "value": "test" } }),
        )],
    )
    .await;
    let config = dump(&backend).await;
    assert_eq!(config.plugin_metadata.unwrap().get(metadata1_name).and_then(|v| v.get("test")), Some(&json!({ "value": "test" })));

    sync_ok(&backend, vec![delete(ResourceType::PluginMetadata, metadata1_name)]).await;
    let config = dump(&backend).await;
    let metadata = config.plugin_metadata.unwrap();
    assert_eq!(metadata.len(), 1);
    assert!(metadata.contains_key(metadata2_name));

    sync_ok(&backend, vec![delete(ResourceType::PluginMetadata, metadata2_name)]).await;
    let config = dump(&backend).await;
    assert!(config.plugin_metadata.is_none() || config.plugin_metadata.unwrap().is_empty());
}
