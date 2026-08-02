//! Real end-to-end tests against a live APISIX instance, not a mock.
//! Requires `docker compose up -d` in `libs/backend-apisix/e2e/assets`
//! (the same stack the TS `backend-apisix` e2e suite uses) — admin API at
//! `http://localhost:19180`, admin key `edd1c9f034335f136f87ad84b625c8f1`.
//!
//! Ignored by default (`cargo test` never touches the network); run with
//! `cargo test -p adc-backend-apisix --test e2e_apisix -- --ignored --test-threads=1`.
//! Single-threaded because tests share one APISIX/etcd instance and each
//! cleans up its own resources rather than sandboxing into a namespace.

use adc_backend_apisix::tests::{Fetcher, Operator};
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::{BackendSyncOptions, Event, EventKind, ResourceType};
use semver::Version;
use serde_json::json;

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn read_asset(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../libs/backend-apisix/e2e/assets").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The CI matrix runs this suite against every supported APISIX release
/// (`BACKEND_APISIX_VERSION`, same env var the TS e2e suite reads) — falls
/// back to a version high enough to exercise every version-gated code path
/// when unset, for local runs against whatever's in the compose file.
fn apisix_version() -> Version {
    std::env::var("BACKEND_APISIX_VERSION").ok().and_then(|v| Version::parse(&v).ok()).unwrap_or(Version::new(999, 999, 999))
}

fn client() -> HttpClient {
    HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap()
}

fn operator() -> Operator {
    Operator::new(client(), apisix_version())
}

fn fetcher() -> Fetcher {
    Fetcher::new(client(), apisix_version())
}

fn create(rt: ResourceType, id: &str, new_value: serde_json::Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, id, id)
}

fn delete(rt: ResourceType, id: &str) -> Event {
    Event::new(rt, EventKind::Delete { old_value: json!({}) }, id, id)
}

fn delete_child(rt: ResourceType, id: &str, parent_id: &str) -> Event {
    let mut event = delete(rt, id);
    event.parent_id = Some(parent_id.to_string());
    event
}

async fn sync_ok(events: Vec<Event>) {
    let results = operator().sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "sync failed for {:?} {}: {:?}", result.event.resource_type, result.event.resource_id, result.error);
    }
}

#[tokio::test]
#[ignore]
async fn syncs_a_service_with_upstream_and_route_then_reads_them_back() {
    let service_id = "e2e-svc-1";
    let route_id = "e2e-route-1";

    let mut route_event = create(ResourceType::Route, route_id, json!({ "name": "e2e route", "uris": ["/e2e-1"] }));
    route_event.parent_id = Some(service_id.to_string());

    sync_ok(vec![
        create(ResourceType::Service, service_id, json!({ "name": "e2e service", "upstream": { "nodes": [{ "host": "127.0.0.1", "port": 1980, "weight": 1 }] } })),
        route_event,
    ])
    .await;

    let services = fetcher().list_services().await.unwrap();
    let service = services.iter().find(|s| s.id == service_id).expect("service was not written");
    assert_eq!(service.name.as_deref(), Some("e2e service"));
    assert_eq!(service.upstream_id.as_deref(), Some(service_id));

    let upstreams = fetcher().list_upstreams().await.unwrap();
    let upstream = upstreams.iter().find(|u| u.id.as_deref() == Some(service_id)).expect("upstream was not written");
    let adc_upstream: adc_sdk::resources::Upstream = upstream.clone().try_into().unwrap();
    let nodes = adc_upstream.nodes.unwrap();
    assert_eq!(nodes[0].host, "127.0.0.1");
    assert_eq!(nodes[0].port, 1980);

    let routes = fetcher().list_routes().await.unwrap();
    let route = routes.iter().find(|r| r.id == route_id).expect("route was not written");
    assert_eq!(route.uris, Some(vec!["/e2e-1".to_string()]));
    assert_eq!(route.service_id.as_deref(), Some(service_id));

    sync_ok(vec![delete(ResourceType::Route, route_id), delete(ResourceType::Service, service_id)]).await;

    let routes = fetcher().list_routes().await.unwrap();
    assert!(routes.iter().all(|r| r.id != route_id), "route should have been deleted");
    let upstreams = fetcher().list_upstreams().await.unwrap();
    assert!(upstreams.iter().all(|u| u.id.as_deref() != Some(service_id)), "upstream should have been deleted alongside its service");
}

#[tokio::test]
#[ignore]
async fn syncs_an_ssl_certificate_then_reads_it_back() {
    let cert = read_asset("test-ssl.cer");
    let key = read_asset("test-ssl.key");
    let ssl_id = "e2e-ssl-1";

    sync_ok(vec![create(
        ResourceType::Ssl,
        ssl_id,
        json!({ "snis": ["e2e.example.com"], "certificates": [{ "certificate": cert, "key": key }] }),
    )])
    .await;

    let ssls = fetcher().list_ssls().await.unwrap();
    let ssl = ssls.iter().find(|s| s.id == ssl_id).expect("ssl was not written");
    assert_eq!(ssl.snis.as_deref(), Some(&["e2e.example.com".to_string()][..]));
    assert!(ssl.cert.is_some());

    sync_ok(vec![delete(ResourceType::Ssl, ssl_id)]).await;
    let ssls = fetcher().list_ssls().await.unwrap();
    assert!(ssls.iter().all(|s| s.id != ssl_id));
}

#[tokio::test]
#[ignore]
async fn syncs_a_consumer_with_a_key_auth_credential_then_reads_it_back() {
    if apisix_version() < Version::new(3, 11, 0) {
        eprintln!("skipping: consumer credentials require apisix >= 3.11.0");
        return;
    }

    // APISIX's consumer `username` pattern is stricter than most other id
    // fields on older versions (`^[a-zA-Z0-9_]+$`, no hyphens) — 3.17.0
    // happens to accept hyphens too, but keep this hyphen-free so the test
    // passes across the whole version matrix.
    let username = "e2e_consumer_1";
    let credential_id = "e2e-cred-1";

    let mut credential_event =
        create(ResourceType::ConsumerCredential, credential_id, json!({ "name": credential_id, "type": "key-auth", "config": { "key": "e2e-secret" } }));
    credential_event.parent_id = Some(username.to_string());

    sync_ok(vec![create(ResourceType::Consumer, username, json!({ "username": username })), credential_event]).await;

    let consumers = fetcher().list_consumers().await.unwrap();
    let consumer = consumers.iter().find(|c| c.username == username).expect("consumer was not written");
    let credentials = consumer.credentials.as_ref().expect("credentials should have been fetched (version-gated above)");
    assert!(credentials.iter().any(|c| c.id.as_deref() == Some(credential_id)));

    sync_ok(vec![delete_child(ResourceType::ConsumerCredential, credential_id, username), delete(ResourceType::Consumer, username)]).await;
    let consumers = fetcher().list_consumers().await.unwrap();
    assert!(consumers.iter().all(|c| c.username != username));
}

#[tokio::test]
#[ignore]
async fn syncs_a_stream_route_then_reads_it_back() {
    if apisix_version() < Version::new(3, 7, 0) {
        eprintln!("skipping: stream routes require apisix >= 3.7.0");
        return;
    }

    let service_id = "e2e-svc-stream-1";
    let stream_route_id = "e2e-stream-route-1";

    let mut stream_route_event =
        create(ResourceType::StreamRoute, stream_route_id, json!({ "name": "e2e-stream-route", "server_port": 33061 }));
    stream_route_event.parent_id = Some(service_id.to_string());

    sync_ok(vec![
        create(ResourceType::Service, service_id, json!({ "name": "e2e stream service", "upstream": { "nodes": [{ "host": "127.0.0.1", "port": 1980, "weight": 1 }] } })),
        stream_route_event,
    ])
    .await;

    let stream_routes = fetcher().list_stream_routes().await.unwrap();
    let route = stream_routes.iter().find(|r| r.id.as_deref() == Some(stream_route_id)).expect("stream route was not written");
    assert_eq!(route.server_port, Some(33061));
    let adc_route: adc_sdk::resources::StreamRoute = route.clone().into();
    if apisix_version() >= Version::new(3, 8, 0) {
        // Recovered from the __ADC_NAME label injected on write (APISIX
        // stream routes have no native `name` field, and that label is only
        // written from 3.8.0 on) — proves the read/write round trip for
        // that trick actually works against a real server, not just our
        // own mock. Matches the TS suite's own `Dump (>=3.8.0)` case.
        assert_eq!(adc_route.name, "e2e-stream-route");
    } else {
        // Below 3.8.0 no label is ever written, so recovery falls back to
        // the route's own id — matches the TS suite's `Dump (<3.8.0)` case.
        assert_eq!(adc_route.name, stream_route_id);
    }

    sync_ok(vec![delete(ResourceType::StreamRoute, stream_route_id), delete(ResourceType::Service, service_id)]).await;
    let stream_routes = fetcher().list_stream_routes().await.unwrap();
    assert!(stream_routes.iter().all(|r| r.id.as_deref() != Some(stream_route_id)));
}
