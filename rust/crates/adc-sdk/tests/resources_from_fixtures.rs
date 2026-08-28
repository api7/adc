//! Deserializes real resource bodies taken from `fixtures/differ/*.json`
//! (the TS/Rust differ parity fixtures) into the typed `resources::*` structs,
//! to confirm field names/optionality/nesting match what real ADC configs
//! actually look like. Not a test of semantic validation (`.refine()` rules
//! aren't implemented yet) — only that well-formed resource bodies parse.

use std::path::PathBuf;

use adc_sdk::resources::{Consumer, FlatConfiguration, Route, Service, ServiceRoutes, UpstreamHealthCheck, SSL};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/differ").join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn deserializes_service_with_nested_routes_and_upstream() {
    let f = fixture("upstream.creates_and_updates_ssl_before_upstream.json");
    let services = f["local"]["services"].clone();
    let services: Vec<Service> = serde_json::from_value(services).expect("deserialize services");
    assert_eq!(services.len(), 1);

    let service = &services[0];
    assert_eq!(service.name, "test");
    assert_eq!(service.routes.as_ref().and_then(ServiceRoutes::http).map(<[_]>::len), Some(0));
    assert!(service.upstream.is_some());
    let upstreams = service.upstreams.as_ref().expect("upstreams");
    assert_eq!(upstreams.len(), 1);
    assert!(upstreams[0].tls.is_some());
}

#[test]
fn deserializes_ssl_with_certificates() {
    let f = fixture("upstream.creates_and_updates_ssl_before_upstream.json");
    let ssls: Vec<SSL> = serde_json::from_value(f["local"]["ssls"].clone()).expect("deserialize ssls");
    assert_eq!(ssls.len(), 2);
    assert_eq!(ssls[0].snis, vec!["test1.com", "test2.com"]);
    assert_eq!(ssls[0].certificates.len(), 1);
    assert_eq!(ssls[0].certificates[0].key, "KEY");
}

#[test]
fn deserializes_consumer_with_credentials() {
    let f = fixture("consumer.creates_updates_deletes_consumer_credentials.json");
    let consumers: Vec<Consumer> =
        serde_json::from_value(f["local"]["consumers"].clone()).expect("deserialize consumers");
    assert_eq!(consumers.len(), 1);
    let credentials = consumers[0].credentials.as_ref().expect("credentials");
    assert_eq!(credentials.len(), 2);
    assert_eq!(credentials[0].r#type, "key-auth");
}

#[test]
fn deserializes_route_with_plugins_and_methods() {
    let f = fixture("usecase.renames_service_with_nested_routes.json");
    let routes = f["local"]["services"][0]["routes"].clone();
    let routes: Vec<Route> = serde_json::from_value(routes).expect("deserialize routes");
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].name, "Anything");
    assert_eq!(routes[0].uris, vec!["/anything"]);
}

/// The default-value patch in this fixture happens to contain a fully
/// populated, realistic `checks` block (active+passive, healthy+unhealthy on
/// both) — good deep coverage for the health-check nesting. Deserializing it
/// as a standalone `UpstreamHealthCheck` (rather than the whole `Upstream`)
/// is deliberate: default-value patches are partial merge patches (this same
/// fixture's `nodes: [{ "priority": 0 }]` sibling field has no host/port/
/// weight), so they can't satisfy a full `Upstream`'s required fields.
#[test]
fn deserializes_full_health_check_block() {
    let f = fixture("usecase.selectively_merges_objects_in_default_values_on_a_service.json");
    let checks_patch = f["defaultValue"]["core"]["service"]["upstream"]["checks"].clone();
    let checks: UpstreamHealthCheck = serde_json::from_value(checks_patch).expect("deserialize checks");

    assert_eq!(checks.active.concurrency, 10);
    let active_healthy = checks.active.healthy.expect("active.healthy");
    assert_eq!(active_healthy.successes, 2);
    let passive = checks.passive.expect("passive");
    let passive_healthy = passive.healthy.expect("passive.healthy");
    assert_eq!(passive_healthy.successes, 5);
}

#[test]
fn deserializes_whole_flat_configuration() {
    let f = fixture("upstream.creates_and_updates_ssl_before_upstream.json");
    let local: FlatConfiguration = serde_json::from_value(f["local"].clone()).expect("deserialize local");
    assert_eq!(local.services.map(|s| s.len()), Some(1));
    assert_eq!(local.ssls.map(|s| s.len()), Some(2));

    let remote: FlatConfiguration = serde_json::from_value(f["remote"].clone()).expect("deserialize remote");
    assert_eq!(remote.services.map(|s| s.len()), Some(1));
    assert_eq!(remote.ssls.map(|s| s.len()), Some(1));
}
