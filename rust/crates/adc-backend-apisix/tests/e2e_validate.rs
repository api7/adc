//! Ported from `libs/backend-apisix/e2e/validate.e2e-spec.ts`. Real network
//! calls against a live APISIX (>= 3.17.0, same as the TS suite's own
//! version gate) — see `e2e_apisix.rs`'s module doc for how to bring one up
//! and run this file.
//!
//! Not ported: the TS "bad uri type" case (`uris: [123 as unknown as
//! string]`) — it relies on bypassing TypeScript's compile-time check to
//! smuggle a non-string into the array. `adc_sdk::resources::Route.uris` is
//! `Vec<String>`, so that specific malformed shape can't be constructed at
//! all; there's nothing to send.

use adc_backend_apisix::tests::Validator;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::{Event, EventKind, ResourceType};
use serde_json::json;

const SERVER: &str = "http://localhost:19180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

/// `/apisix/admin/configs/validate` doesn't exist before 3.17.0 (a request
/// gets a 404, surfaced as `BackendError::Unsupported`) — matches the TS
/// suite's own `conditionalDescribe(semverCondition(gte, '3.17.0'))` gate.
fn apisix_version() -> semver::Version {
    std::env::var("BACKEND_APISIX_VERSION").ok().and_then(|v| semver::Version::parse(&v).ok()).unwrap_or(semver::Version::new(999, 999, 999))
}

fn validator() -> Validator {
    let client = HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap();
    Validator::new(client)
}

fn create(rt: ResourceType, id: &str, new_value: serde_json::Value) -> Event {
    Event::new(rt, EventKind::Create { new_value }, id, id)
}

/// A service create event plus its route's create event, linked via
/// `parent_id` the way the differ would produce them.
fn service_with_route(service_id: &str, route_id: &str, route: serde_json::Value) -> Vec<Event> {
    let service = create(
        ResourceType::Service,
        service_id,
        json!({ "name": service_id, "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] } }),
    );
    let mut route_event = create(ResourceType::Route, route_id, route);
    route_event.parent_id = Some(service_id.to_string());
    vec![service, route_event]
}

#[tokio::test]
#[ignore]
async fn succeeds_with_an_empty_configuration() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    let result = validator().validate(&[]).await.unwrap();
    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn succeeds_with_a_valid_service_and_route() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    let events = service_with_route(
        "e2e-validate-svc1",
        "e2e-validate-route1",
        json!({ "name": "validate-test-route", "uris": ["/validate-test"], "methods": ["GET"] }),
    );

    let result = validator().validate(&events).await.unwrap();
    assert!(result.success, "{:?}", result.errors);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn succeeds_with_a_valid_consumer() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    let events = vec![create(
        ResourceType::Consumer,
        "validate-test-consumer",
        json!({ "username": "validate-test-consumer", "plugins": { "key-auth": { "key": "test-key-123" } } }),
    )];

    let result = validator().validate(&events).await.unwrap();
    assert!(result.success, "{:?}", result.errors);
}

#[tokio::test]
#[ignore]
async fn fails_with_an_invalid_plugin_configuration() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    // limit-count requires `count`/`time_window`; both are missing.
    let events = service_with_route(
        "e2e-validate-svc2",
        "e2e-validate-route2",
        json!({ "name": "validate-bad-plugin-route", "uris": ["/bad-plugin"], "plugins": { "limit-count": {} } }),
    );

    let result = validator().validate(&events).await.unwrap();
    assert!(!result.success);
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].resource_type, "routes");
    // The error is mapped back to the specific Event that produced it, not
    // just its position in apisix's response.
    assert_eq!(result.errors[0].resource_name.as_deref(), Some("e2e-validate-route2"));
    let matched_event = result.errors[0].event.as_ref().expect("event should have been matched from the request index");
    assert_eq!(matched_event.resource_type, ResourceType::Route);
    assert_eq!(matched_event.resource_id, "e2e-validate-route2");
    assert_eq!(matched_event.parent_id.as_deref(), Some("e2e-validate-svc2"));
}

#[tokio::test]
#[ignore]
async fn collects_multiple_errors() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    let service = create(
        ResourceType::Service,
        "e2e-validate-svc3",
        json!({ "name": "validate-multi-err-svc", "upstream": { "scheme": "http", "nodes": [{ "host": "httpbin.org", "port": 80, "weight": 100 }] } }),
    );
    let mut route1 = create(
        ResourceType::Route,
        "e2e-validate-route3a",
        json!({ "name": "validate-multi-err-route1", "uris": ["/multi-err-1"], "plugins": { "limit-count": {} } }),
    );
    route1.parent_id = Some("e2e-validate-svc3".to_string());
    let mut route2 = create(
        ResourceType::Route,
        "e2e-validate-route3b",
        json!({ "name": "validate-multi-err-route2", "uris": ["/multi-err-2"], "plugins": { "limit-count": {} } }),
    );
    route2.parent_id = Some("e2e-validate-svc3".to_string());

    let result = validator().validate(&[service, route1, route2]).await.unwrap();
    assert!(!result.success);
    assert!(result.errors.len() >= 2, "{:?}", result.errors);
    // Each route's error maps back to *its own* name, not a mix-up between
    // the two routes sharing a parent service.
    let names: Vec<&str> = result.errors.iter().filter_map(|e| e.resource_name.as_deref()).collect();
    assert!(names.contains(&"e2e-validate-route3a"), "{names:?}");
    assert!(names.contains(&"e2e-validate-route3b"), "{names:?}");
}

#[tokio::test]
#[ignore]
async fn succeeds_with_mixed_resource_types() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    let mut events = service_with_route(
        "e2e-validate-svc4",
        "e2e-validate-route4",
        json!({ "name": "validate-mixed-route", "uris": ["/mixed-test"], "methods": ["GET", "POST"] }),
    );
    events.push(create(
        ResourceType::Consumer,
        "validate-mixed-consumer",
        json!({ "username": "validate-mixed-consumer", "plugins": { "key-auth": { "key": "mixed-key-456" } } }),
    ));
    events.push(create(ResourceType::GlobalRule, "prometheus", json!({ "prefer_name": false })));

    let result = validator().validate(&events).await.unwrap();
    assert!(result.success, "{:?}", result.errors);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn is_a_dry_run_with_no_side_effects_on_the_server() {
    if apisix_version() < semver::Version::new(3, 17, 0) {
        eprintln!("skipping: validate requires apisix >= 3.17.0");
        return;
    }

    let service_id = "e2e-validate-dryrun-svc";
    let events = service_with_route(
        service_id,
        "e2e-validate-dryrun-route",
        json!({ "name": "validate-dryrun-route", "uris": ["/dryrun-test"] }),
    );

    let result = validator().validate(&events).await.unwrap();
    assert!(result.success, "{:?}", result.errors);

    let client = HttpClient::new(HttpClientConfig { server: SERVER.to_string(), token: TOKEN.to_string(), timeout: None, tls: TlsConfig::default() }).unwrap();
    let fetcher = adc_backend_apisix::tests::Fetcher::new(client, semver::Version::new(3, 17, 0));
    let services = fetcher.list_services().await.unwrap();
    assert!(services.iter().all(|s| s.id != service_id), "validate must not have created anything on the server");
}
