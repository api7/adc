//! Cross-resource-type isolation: a single sync that touches only one
//! resource type must leave every other type's `modifiedIndex`/
//! `*_conf_version` byte-for-byte unchanged. The other e2e suites each check
//! this pairwise, for whichever two types their own scenario happens to
//! involve (a service vs. its inline upstream, one plugin_metadata entry vs.
//! another); this one seeds one resource of all 8 wire collections at once
//! and asserts the *entire rest of the document* is untouched after a change
//! to just one of them — the full matrix, not a handful of pairs. No TS
//! reference spec to port from. Real network calls against a live
//! 3-instance standalone APISIX cluster — see `common`'s module doc for how
//! to bring one up and run this file.

use adc_backend_apisix_standalone::tests::typing::ApisixStandalone;
use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;
use serde_json::json;

mod common;
use common::{backend, base_service, base_upstream, diff, empty_configuration};

const TEST_CERT: &str = include_str!("../../../../libs/backend-apisix-standalone/e2e/assets/test-ssl.cer");
const TEST_KEY: &str = include_str!("../../../../libs/backend-apisix-standalone/e2e/assets/test-ssl.key");

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn node(port: u16) -> adc::UpstreamNode {
    adc::UpstreamNode { host: "127.0.0.1".to_string(), port, weight: 100, priority: 0, metadata: None }
}

fn route(name: &str) -> adc::Route {
    adc::Route {
        id: None,
        name: name.to_string(),
        description: None,
        labels: None,
        hosts: None,
        uris: vec![format!("/{name}")],
        priority: None,
        timeout: None,
        vars: None,
        methods: None,
        enable_websocket: None,
        remote_addrs: None,
        plugins: None,
        filter_func: None,
    }
}

fn stream_route(name: &str, server_port: u16) -> adc::StreamRoute {
    adc::StreamRoute {
        id: None,
        name: name.to_string(),
        description: None,
        labels: None,
        plugins: None,
        remote_addr: None,
        server_addr: None,
        server_port: Some(server_port),
        sni: None,
    }
}

/// One resource of every wire collection `WireVersions`/`stamp_versions`
/// track: `services` (x2: one http, one stream), `routes`, `stream_routes`,
/// `upstreams` (x3: two inline defaults + one named), `consumers` +
/// (folded into the same collection) `consumer_credentials`, `ssls`,
/// `global_rules`, `plugin_metadata`.
fn full_configuration() -> Configuration {
    let http_service = adc::Service {
        name: "svc-http".to_string(),
        upstream: Some(adc::Upstream { nodes: Some(vec![node(9180)]), ..base_upstream() }),
        upstreams: Some(vec![adc::Upstream { name: Some("nd-upstream1".to_string()), nodes: Some(vec![node(9181)]), ..base_upstream() }]),
        routes: Some(adc::ServiceRoutes::Http { routes: vec![route("route-a")] }),
        ..base_service()
    };
    let stream_service = adc::Service {
        name: "svc-stream".to_string(),
        upstream: Some(adc::Upstream { nodes: Some(vec![node(9280)]), ..base_upstream() }),
        routes: Some(adc::ServiceRoutes::Stream { stream_routes: vec![stream_route("stream-a", 9300)] }),
        ..base_service()
    };

    let mut credential_config = adc::Plugin::new();
    credential_config.insert("key".to_string(), json!("alice-key"));
    let consumer = adc::Consumer {
        username: "alice".to_string(),
        description: None,
        labels: None,
        plugins: Some(adc::Plugins::new()),
        credentials: Some(vec![adc::ConsumerCredential {
            id: None,
            name: "alice-key".to_string(),
            description: None,
            labels: None,
            r#type: "key-auth".to_string(),
            config: credential_config,
        }]),
    };

    let ssl = adc::SSL {
        id: None,
        labels: None,
        r#type: adc::SslType::default(),
        snis: vec!["isolation.example.com".to_string()],
        certificates: vec![adc::SSLCertificate { certificate: TEST_CERT.to_string(), key: TEST_KEY.to_string() }],
        client: None,
        ssl_protocols: None,
    };

    let mut global_rules = adc::Plugins::new();
    global_rules.insert("request-id".to_string(), json!({}));

    let mut plugin_metadata = adc::Plugins::new();
    plugin_metadata.insert("http-logger".to_string(), json!({ "log_format": { "host": "$host" } }));

    Configuration {
        services: Some(vec![http_service, stream_service]),
        ssls: Some(vec![ssl]),
        consumers: Some(vec![consumer]),
        consumer_groups: None,
        global_rules: Some(global_rules),
        plugin_metadata: Some(plugin_metadata),
    }
}

#[tokio::test]
#[ignore]
async fn changing_one_resource_type_leaves_every_other_type_byte_identical() {
    common::restart_apisix().await;
    let backend = backend("isolation-e2e");
    dump(&backend).await;

    // --- Seed one resource of every type. ---
    let before = dump(&backend).await;
    let local = full_configuration();
    let events = diff(&local, &before);
    sync_ok(&backend, events).await;

    let snapshot1: ApisixStandalone = common::raw_config().await;
    // Sanity: every collection this test cares about actually got seeded —
    // an empty one would make the "untouched" assertion below vacuous.
    assert_eq!(snapshot1.services.len(), 2);
    assert_eq!(snapshot1.routes.len(), 1);
    assert_eq!(snapshot1.stream_routes.len(), 1);
    assert_eq!(snapshot1.upstreams.len(), 3, "2 inline defaults + 1 named");
    assert_eq!(snapshot1.consumers.len(), 2, "1 consumer + 1 credential");
    assert_eq!(snapshot1.ssls.len(), 1);
    assert_eq!(snapshot1.global_rules.len(), 1);
    assert_eq!(snapshot1.plugin_metadata.len(), 1);

    // --- Change only the global rule; every other resource type must come
    //     back byte-identical, not just "conf_version didn't move" but the
    //     full collection (content and every member's modifiedIndex). ---
    let before = dump(&backend).await;
    let mut updated = local.clone();
    let mut global_rules = adc::Plugins::new();
    global_rules.insert("request-id".to_string(), json!({ "header_name": "X-Trace-Id" }));
    updated.global_rules = Some(global_rules);
    let events = diff(&updated, &before);
    assert_eq!(events.len(), 1, "only the global rule changed");
    assert_eq!(events[0].resource_type, adc_sdk::ResourceType::GlobalRule);
    sync_ok(&backend, events).await;

    let snapshot2: ApisixStandalone = common::raw_config().await;

    assert!(snapshot2.global_rules_conf_version > snapshot1.global_rules_conf_version, "the changed type's conf_version must bump");
    assert!(
        snapshot2.global_rules[0].modified_index > snapshot1.global_rules[0].modified_index,
        "the changed resource's own modifiedIndex must bump"
    );

    // Rebuild `snapshot2` with the global-rule fields swapped back to
    // `snapshot1`'s — if that reconstruction equals `snapshot1` exactly,
    // nothing else in the document moved at all.
    let snapshot2_with_old_global_rules =
        ApisixStandalone { global_rules: snapshot1.global_rules.clone(), global_rules_conf_version: snapshot1.global_rules_conf_version, ..snapshot2 };
    assert_eq!(
        snapshot2_with_old_global_rules, snapshot1,
        "every resource type other than global_rules — content and modifiedIndex/conf_version alike — must be untouched"
    );
}

/// The no-op-resync guarantee (`resyncing_with_an_empty_changeset_produces_a_
/// byte_identical_document`, unit-tested against a single hand-built
/// document in `operator.rs`) across the full 8-collection matrix a real
/// cluster actually stores, repeated more than once — a one-shot check
/// can't rule out something drifting only on the *second* no-op sync.
#[tokio::test]
#[ignore]
async fn repeated_noop_syncs_leave_the_whole_document_byte_identical() {
    common::restart_apisix().await;
    let backend = backend("isolation-noop-e2e");
    dump(&backend).await;

    let before = dump(&backend).await;
    let local = full_configuration();
    sync_ok(&backend, diff(&local, &before)).await;

    let snapshot: ApisixStandalone = common::raw_config().await;
    for round in 1..=3 {
        let before = dump(&backend).await;
        let events = diff(&local, &before);
        assert!(events.is_empty(), "round {round}: an unchanged desired state must diff to no events");
        sync_ok(&backend, events).await;
        assert_eq!(common::raw_config().await, snapshot, "round {round}: a no-op sync must not move anything");
    }
}

/// Deleting every seeded resource type in one sync: every collection must
/// come back an explicit `[]` on the wire (not merely absent — `#[serde(
/// default)]` on `ApisixStandalone`'s non-`Option` fields can't tell those
/// apart, so this checks the untyped JSON directly), and every collection
/// that actually held something bumps its `conf_version` to the *same*
/// shared timestamp — one sync, one moment, not staggered per type.
#[tokio::test]
#[ignore]
async fn deleting_everything_at_once_clears_every_collection_with_one_shared_timestamp() {
    common::restart_apisix().await;
    let backend = backend("isolation-delete-all-e2e");
    dump(&backend).await;

    let before = dump(&backend).await;
    let local = full_configuration();
    sync_ok(&backend, diff(&local, &before)).await;

    let before = dump(&backend).await;
    let events = diff(&empty_configuration(), &before);
    sync_ok(&backend, events).await;

    let raw: ApisixStandalone = common::raw_config().await;
    assert_eq!(raw.services.len(), 0);
    assert_eq!(raw.routes.len(), 0);
    assert_eq!(raw.stream_routes.len(), 0);
    assert_eq!(raw.upstreams.len(), 0);
    assert_eq!(raw.consumers.len(), 0);
    assert_eq!(raw.ssls.len(), 0);
    assert_eq!(raw.global_rules.len(), 0);
    assert_eq!(raw.plugin_metadata.len(), 0);

    let timestamp = raw.services_conf_version;
    assert!(timestamp > 0);
    for (field, version) in [
        ("services_conf_version", raw.services_conf_version),
        ("routes_conf_version", raw.routes_conf_version),
        ("stream_routes_conf_version", raw.stream_routes_conf_version),
        ("upstreams_conf_version", raw.upstreams_conf_version),
        ("consumers_conf_version", raw.consumers_conf_version),
        ("ssls_conf_version", raw.ssls_conf_version),
        ("global_rules_conf_version", raw.global_rules_conf_version),
        ("plugin_metadata_conf_version", raw.plugin_metadata_conf_version),
    ] {
        assert_eq!(version, timestamp, "{field} must share the one timestamp this delete-everything sync stamped");
    }
}
