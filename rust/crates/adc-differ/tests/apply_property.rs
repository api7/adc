//! `apply` is supposed to be the structural inverse of `DifferV4::diff`:
//! `apply(diff(local, remote), remote)` must reconstruct `local` (modulo
//! array order and the `Some(vec![])`/absent-field equivalence `diff`
//! itself already treats as one thing — see `canonicalize` below). This is
//! the property `Operator::sync`'s planned redesign leans on to rebuild the
//! full desired config from a cached baseline plus a differ's events,
//! without ever holding onto `local` itself between syncs — see
//! `adc_differ::apply`'s doc comment.
//!
//! Every resource here carries an explicit, deterministically-derived `id`
//! (or `username`, for `Consumer`) on both `local` and `remote` — matching
//! what `generate_id`/`generate_id_with_parent` would derive from the same
//! name anyway (`DifferV4` strips `id` before ever comparing or diffing an
//! item, so this changes nothing about what events come out), and letting
//! this test control, via the shared name pools below, which of `local` and
//! `remote`'s items are meant to be "the same resource" (shared id ->
//! Update or no-op) versus genuinely different ones (Create/Delete).

use adc_differ::DifferV4;
use adc_sdk::resources::FlatConfiguration;
use adc_sdk::utils::generate_id;
use proptest::prelude::*;
use serde_json::{Map, Value, json};

const SERVICE_NAMES: [&str; 3] = ["svc-a", "svc-b", "svc-c"];
const ROUTE_NAMES: [&str; 3] = ["r1", "r2", "r3"];
const UPSTREAM_NAMES: [&str; 2] = ["u1", "u2"];
const CONSUMER_NAMES: [&str; 3] = ["alice", "bob", "carol"];
const CREDENTIAL_NAMES: [&str; 2] = ["c1", "c2"];
const SSL_KEYS: [&str; 2] = ["ssl1", "ssl2"];
const PLUGIN_NAMES: [&str; 2] = ["limit-count", "key-auth"];

fn variant() -> impl Strategy<Value = u8> {
    0u8..3
}

fn make_route(parent_id: &str, name: &str, is_stream: bool, v: u8) -> Value {
    let id = generate_id(&format!("{parent_id}.{name}"));
    if is_stream {
        json!({ "id": id, "name": name, "sni": format!("v{v}.example.com") })
    } else {
        json!({ "id": id, "name": name, "uris": [format!("/p{v}")] })
    }
}

fn make_named_upstream(parent_id: &str, name: &str, v: u8) -> Value {
    json!({
        "id": generate_id(&format!("{parent_id}.{name}")),
        "name": name,
        "nodes": [{ "host": format!("h{v}.example.com"), "port": 80, "weight": 1, "priority": 0 }],
    })
}

fn make_default_upstream(v: u8) -> Value {
    json!({ "nodes": [{ "host": format!("h{v}.example.com"), "port": 80, "weight": 1, "priority": 0 }] })
}

fn make_credential(parent_id: &str, name: &str, v: u8) -> Value {
    json!({
        "id": generate_id(&format!("{parent_id}.{name}")),
        "name": name,
        "type": "key-auth",
        "config": { "key": format!("k{v}") },
    })
}

fn make_ssl(key: &str, v: u8) -> Value {
    json!({
        "id": generate_id(key),
        "snis": [key],
        "certificates": [{ "certificate": format!("cert-{v}"), "key": format!("key-{v}") }],
    })
}

fn make_plugin_config(v: u8) -> Value {
    json!({ "count": v })
}

/// `routes_kind`: `0` = neither, `1` = HTTP routes, `2` = stream routes —
/// `Service.routes`/`Service.stream_routes` are mutually exclusive (see
/// `ServiceRoutes`), so this is one slot, not two independent `Option`s.
fn service(name: &'static str) -> impl Strategy<Value = Value> {
    let id = generate_id(name);
    (
        proptest::option::of(variant()),
        0u8..3,
        proptest::option::of(variant()),
        proptest::option::of(variant()),
        proptest::option::of(variant()),
        proptest::option::of(variant()),
        proptest::option::of(variant()),
    )
        .prop_map(move |(upstream, routes_kind, r1, r2, r3, u1, u2)| {
            let mut obj = Map::new();
            obj.insert("id".to_string(), json!(id));
            obj.insert("name".to_string(), json!(name));
            if let Some(v) = upstream {
                obj.insert("upstream".to_string(), make_default_upstream(v));
            }

            let upstreams: Vec<Value> = [(UPSTREAM_NAMES[0], u1), (UPSTREAM_NAMES[1], u2)]
                .into_iter()
                .filter_map(|(name, v)| v.map(|v| make_named_upstream(&id, name, v)))
                .collect();
            if !upstreams.is_empty() {
                obj.insert("upstreams".to_string(), json!(upstreams));
            }

            if routes_kind != 0 {
                let is_stream = routes_kind == 2;
                let routes: Vec<Value> = [(ROUTE_NAMES[0], r1), (ROUTE_NAMES[1], r2), (ROUTE_NAMES[2], r3)]
                    .into_iter()
                    .filter_map(|(name, v)| v.map(|v| make_route(&id, name, is_stream, v)))
                    .collect();
                if !routes.is_empty() {
                    let key = if is_stream { "stream_routes" } else { "routes" };
                    obj.insert(key.to_string(), json!(routes));
                }
            }
            Value::Object(obj)
        })
}

fn consumer(username: &'static str) -> impl Strategy<Value = Value> {
    (proptest::option::of(variant()), proptest::option::of(variant())).prop_map(move |(c1, c2)| {
        let mut obj = Map::new();
        obj.insert("username".to_string(), json!(username));
        let credentials: Vec<Value> = [(CREDENTIAL_NAMES[0], c1), (CREDENTIAL_NAMES[1], c2)]
            .into_iter()
            .filter_map(|(name, v)| v.map(|v| make_credential(username, name, v)))
            .collect();
        if !credentials.is_empty() {
            obj.insert("credentials".to_string(), json!(credentials));
        }
        Value::Object(obj)
    })
}

fn ssl(key: &'static str) -> impl Strategy<Value = Value> {
    variant().prop_map(move |v| make_ssl(key, v))
}

/// A whole config: 0-3 services (each with an optional default upstream, 0-2
/// named upstreams, and either 0-3 HTTP routes or 0-3 stream routes), 0-3
/// consumers (each with 0-2 credentials), 0-2 SSLs, 0-2 global rules, 0-2
/// plugin_metadata entries — small enough for proptest to explore
/// exhaustively-ish while covering every `CollectionKind` (`Array`/`Record`)
/// and nesting shape (`Service`'s 3 nested fields, `Consumer`'s 1) `apply`
/// has to handle.
fn config() -> impl Strategy<Value = FlatConfiguration> {
    let services = (
        proptest::option::of(service(SERVICE_NAMES[0])),
        proptest::option::of(service(SERVICE_NAMES[1])),
        proptest::option::of(service(SERVICE_NAMES[2])),
    );
    let consumers = (
        proptest::option::of(consumer(CONSUMER_NAMES[0])),
        proptest::option::of(consumer(CONSUMER_NAMES[1])),
        proptest::option::of(consumer(CONSUMER_NAMES[2])),
    );
    let ssls = (proptest::option::of(ssl(SSL_KEYS[0])), proptest::option::of(ssl(SSL_KEYS[1])));
    let global_rules = (proptest::option::of(variant()), proptest::option::of(variant()));
    let plugin_metadata = (proptest::option::of(variant()), proptest::option::of(variant()));

    (services, consumers, ssls, global_rules, plugin_metadata).prop_map(
        |((s1, s2, s3), (c1, c2, c3), (ssl1, ssl2), (gr1, gr2), (pm1, pm2))| {
            let mut obj = Map::new();

            let services: Vec<Value> = [s1, s2, s3].into_iter().flatten().collect();
            if !services.is_empty() {
                obj.insert("services".to_string(), json!(services));
            }

            let consumers: Vec<Value> = [c1, c2, c3].into_iter().flatten().collect();
            if !consumers.is_empty() {
                obj.insert("consumers".to_string(), json!(consumers));
            }

            let ssls: Vec<Value> = [ssl1, ssl2].into_iter().flatten().collect();
            if !ssls.is_empty() {
                obj.insert("ssls".to_string(), json!(ssls));
            }

            let global_rules: Map<String, Value> = [(PLUGIN_NAMES[0], gr1), (PLUGIN_NAMES[1], gr2)]
                .into_iter()
                .filter_map(|(name, v)| v.map(|v| (name.to_string(), make_plugin_config(v))))
                .collect();
            if !global_rules.is_empty() {
                obj.insert("global_rules".to_string(), Value::Object(global_rules));
            }

            let plugin_metadata: Map<String, Value> = [(PLUGIN_NAMES[0], pm1), (PLUGIN_NAMES[1], pm2)]
                .into_iter()
                .filter_map(|(name, v)| v.map(|v| (name.to_string(), make_plugin_config(v))))
                .collect();
            if !plugin_metadata.is_empty() {
                obj.insert("plugin_metadata".to_string(), Value::Object(plugin_metadata));
            }

            serde_json::from_value(Value::Object(obj))
                .unwrap_or_else(|e| panic!("strategy produced an invalid FlatConfiguration: {e}"))
        },
    )
}

/// Order-normalizes every array this test's own fixtures can produce, by
/// its identity field (`id`, or `username` for consumers), so
/// `apply`-reconstructed-`local` can be compared against `local` with plain
/// `==` despite `apply` not preserving original array order (`Create`
/// always appends; it doesn't know where an item "should" go). Not a stand-in
/// for `apply`'s own logic — this only needs to know enough about the shape
/// this test itself generates to make comparison order-independent.
fn canonicalize(value: &mut Value) {
    let Value::Object(map) = value else { return };

    for (field, key) in [("services", "id"), ("ssls", "id"), ("consumers", "username")] {
        if let Some(Value::Array(items)) = map.get_mut(field) {
            items.sort_by_key(|item| item_key(item, key));
        }
    }
    for service in map.get_mut("services").into_iter().flat_map(|v| v.as_array_mut()).flatten() {
        let Value::Object(service) = service else { continue };
        for field in ["routes", "stream_routes", "upstreams"] {
            if let Some(Value::Array(items)) = service.get_mut(field) {
                items.sort_by_key(|item| item_key(item, "id"));
            }
        }
    }
    for consumer in map.get_mut("consumers").into_iter().flat_map(|v| v.as_array_mut()).flatten() {
        let Value::Object(consumer) = consumer else { continue };
        if let Some(Value::Array(items)) = consumer.get_mut("credentials") {
            items.sort_by_key(|item| item_key(item, "id"));
        }
    }
}

fn item_key(item: &Value, field: &str) -> String {
    item.get(field).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn as_value(config: &FlatConfiguration) -> Value {
    serde_json::to_value(config).expect("FlatConfiguration always serializes")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// `apply(diff(local, remote), remote) == local`, order-insensitively.
    #[test]
    fn apply_reconstructs_local_from_remote_and_the_diff_between_them(
        local in config(),
        remote in config(),
    ) {
        let events = DifferV4::diff(&local, &remote, None);
        let reconstructed = adc_differ::apply(&events, &remote);

        let mut expected = as_value(&local);
        let mut actual = as_value(&reconstructed);
        canonicalize(&mut expected);
        canonicalize(&mut actual);

        prop_assert_eq!(
            actual, expected,
            "apply(diff(local, remote), remote) != local\nevents: {:#?}",
            events,
        );
    }

    /// A degenerate but important case on its own: diffing a config against
    /// itself yields no events, and applying no events onto a config must
    /// leave it unchanged.
    #[test]
    fn applying_no_events_is_a_no_op(remote in config()) {
        let events = DifferV4::diff(&remote, &remote, None);
        prop_assert!(events.is_empty(), "diffing a config against itself produced events: {events:#?}");

        let reconstructed = adc_differ::apply(&events, &remote);
        let mut expected = as_value(&remote);
        let mut actual = as_value(&reconstructed);
        canonicalize(&mut expected);
        canonicalize(&mut actual);
        prop_assert_eq!(actual, expected);
    }
}

/// Not itself a property test, but pins down (via the same generators
/// proptest drives) that this file's model is actually exercising the
/// interesting cases — creates, updates, deletes, and untouched resources
/// all appearing across the pool overlap between two independently-drawn
/// configs — rather than degenerating into "always empty" or "always
/// identical" by construction.
#[test]
fn the_generators_produce_a_realistic_mix_of_event_types() {
    use adc_sdk::EventType;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, TestRunner};

    let mut runner = TestRunner::new(Config::with_cases(200));
    let (mut creates, mut updates, mut deletes) = (0, 0, 0);
    for _ in 0..200 {
        let local = config().new_tree(&mut runner).unwrap().current();
        let remote = config().new_tree(&mut runner).unwrap().current();
        for event in DifferV4::diff(&local, &remote, None) {
            match event.event_type() {
                EventType::Create => creates += 1,
                EventType::Update => updates += 1,
                EventType::Delete => deletes += 1,
            }
        }
    }
    assert!(creates > 0, "no Create events across 200 sampled pairs");
    assert!(updates > 0, "no Update events across 200 sampled pairs");
    assert!(deletes > 0, "no Delete events across 200 sampled pairs");
}
