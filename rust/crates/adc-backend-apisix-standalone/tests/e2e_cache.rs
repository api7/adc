//! Ported from `libs/backend-apisix-standalone/e2e/cache.e2e-spec.ts`. Real
//! network calls against a live 3-instance standalone APISIX cluster — see
//! `common`'s module doc for how to bring one up and run this file.
//!
//! The TS suite pins `stableTimestamp()` to exact values via `vi.mock` for
//! several assertions; this port has no clock-injection seam (see
//! `e2e_resource_service_inline_upstream.rs`'s module doc for why) and
//! checks the same properties via self-consistency and real wall-clock
//! ordering instead. Not ported: the TS suite's final `axios.get(...)`
//! check per `describe` block that port 9080 (the data plane, not the
//! admin API) also 401s without a key — that exercises APISIX's own HTTP
//! server, not anything in this crate.

use std::time::Duration;

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::{backend, backend_for, backend_multi, base_service, base_upstream, diff, empty_configuration};

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) -> Vec<adc_sdk::BackendSyncResult> {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
    results
}

fn node(host: &str, port: u16) -> adc::UpstreamNode {
    adc::UpstreamNode { host: host.to_string(), port, weight: 100, priority: 0, metadata: None }
}

/// A `service1` (with an inline upstream on `upstream_port`, and a route
/// bound to `/apisix/admin/configs`) plus two empty-plugin consumers,
/// `jack`/`jane` — mirrors the fixture shared by `cache.e2e-spec.ts`'s
/// first three `describe` blocks.
fn fixture_config(upstream_port: u16) -> Configuration {
    let service = adc::Service {
        name: "service1".to_string(),
        upstream: Some(adc::Upstream { nodes: Some(vec![node("127.0.0.1", upstream_port)]), ..base_upstream() }),
        routes: Some(adc::ServiceRoutes::Http {
            routes: vec![adc::Route {
                id: None,
                name: "route1".to_string(),
                description: None,
                labels: None,
                hosts: None,
                uris: vec!["/apisix/admin/configs".to_string()],
                priority: None,
                timeout: None,
                vars: None,
                methods: None,
                enable_websocket: None,
                remote_addrs: None,
                plugins: None,
                filter_func: None,
            }],
        }),
        ..base_service()
    };
    Configuration {
        services: Some(vec![service]),
        consumers: Some(vec![
            adc::Consumer { username: "jack".to_string(), description: None, labels: None, plugins: Some(adc::Plugins::new()), credentials: None },
            adc::Consumer { username: "jane".to_string(), description: None, labels: None, plugins: Some(adc::Plugins::new()), credentials: None },
        ]),
        ..empty_configuration()
    }
}

/// `fixture_config`'s service+route, without the two consumers — matches
/// the smaller `config` object the TS suite's own "Partial new instances"
/// scenario uses (unlike its other scenarios, that one never syncs any
/// consumers at all).
fn service_with_route_config(upstream_port: u16) -> Configuration {
    let mut config = fixture_config(upstream_port);
    config.consumers = None;
    config
}

fn assert_fresh_cache_shape(config: &Configuration) {
    assert!(config.services.is_none(), "a never-configured instance has no services yet");
    assert!(config.ssls.is_none());
    assert!(config.consumers.is_none());
    assert!(config.global_rules.is_none());
    assert!(config.plugin_metadata.is_none());
}

#[tokio::test]
#[ignore]
async fn single_instance_initializes_caches_and_syncs() {
    common::restart_apisix().await;
    let cache_key = "cache-e2e-single";
    let backend = backend(cache_key);

    assert!(common::cache().config(cache_key).await.is_none());
    assert!(common::cache().versions(cache_key).await.is_none());

    let initial = dump(&backend).await;
    assert!(common::cache().config(cache_key).await.is_some());
    let raw = common::raw_config().await;
    assert_fresh_cache_shape(&initial);
    // A never-configured instance reports every conf_version as 0 — the
    // document itself already exists, just empty. `#[serde(default)]`
    // normalizes an older APISIX's stream_routes_conf_version (absent from
    // the document's schema entirely on some older supported versions) to
    // the same 0, so it's asserted the same way as every other type here.
    assert_eq!(raw.routes_conf_version, 0);
    assert_eq!(raw.services_conf_version, 0);
    assert_eq!(raw.consumers_conf_version, 0);
    assert_eq!(raw.ssls_conf_version, 0);
    assert_eq!(raw.global_rules_conf_version, 0);
    assert_eq!(raw.plugin_metadata_conf_version, 0);
    assert_eq!(raw.upstreams_conf_version, 0);
    assert_eq!(raw.stream_routes_conf_version, 0);

    // A second dump is served from cache — same result, no new fetch.
    let again = dump(&backend).await;
    assert_eq!(again, initial);

    let before = dump(&backend).await;
    let local = fixture_config(9180);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 4, "service + route + 2 consumers");
    assert!(events.iter().all(|e| e.event_type() == adc_sdk::EventType::Create));

    let results = sync_ok(&backend, events).await;
    assert_eq!(results.len(), 1, "a single-server backend writes to exactly one server");
    assert_eq!(results[0].server.as_deref(), Some(common::SERVER1));

    let config = common::cache().config(cache_key).await.unwrap();
    let services = config.services.unwrap();
    assert_eq!(services[0].name, "service1");
    let consumers = config.consumers.unwrap();
    assert!(consumers.iter().any(|c| c.username == "jack"));
    assert!(consumers.iter().any(|c| c.username == "jane"));

    // Every resource created in this one sync call shares the same
    // timestamp, on both its own `modifiedIndex` and its collection's
    // `conf_version` — and that's also what got cached as `latest_version`.
    let raw = common::raw_config().await;
    let timestamp = raw.services[0].modified_index;
    assert_eq!(raw.services_conf_version, timestamp);
    assert_eq!(raw.routes[0].modified_index, timestamp);
    assert_eq!(raw.routes_conf_version, timestamp);
    assert_eq!(raw.upstreams[0].modified_index, timestamp);
    assert_eq!(raw.upstreams_conf_version, timestamp);
    for consumer in &raw.consumers {
        let consumer = consumer.as_consumer().expect("this fixture has no credentials, only plain consumers");
        assert_eq!(consumer.modified_index, timestamp);
    }
    assert_eq!(raw.consumers_conf_version, timestamp);
    assert_eq!(common::cache().latest_version(cache_key).await, Some(timestamp));
}

#[tokio::test]
#[ignore]
async fn multiple_fresh_instances_all_receive_the_sync() {
    common::restart_apisix().await;
    let cache_key = "cache-e2e-multi-fresh";
    let backend = backend_multi(cache_key);

    let initial = dump(&backend).await;
    assert_fresh_cache_shape(&initial);
    let raw = common::raw_config().await;
    assert_eq!(raw.services_conf_version, 0);

    let before = dump(&backend).await;
    let events = diff(&fixture_config(9180), &before);
    assert_eq!(events.len(), 4);

    let results = sync_ok(&backend, events).await;
    assert_eq!(results.len(), 3, "a 3-server backend writes to every server");
    let mut servers: Vec<&str> = results.iter().filter_map(|r| r.server.as_deref()).collect();
    servers.sort_unstable();
    assert_eq!(servers, vec![common::SERVER1, common::SERVER2, common::SERVER3]);
}

#[tokio::test]
#[ignore]
async fn a_multi_server_dump_picks_up_whichever_server_was_updated_most_recently() {
    common::restart_apisix().await;
    let cache_key = "cache-e2e-partial";

    // Write independently to server1 (older) ...
    common::cache().invalidate(cache_key).await;
    let backend1 = backend_for(common::SERVER1, cache_key);
    let events = diff(&service_with_route_config(5432), &empty_configuration());
    assert_eq!(events.len(), 2, "service + route; no consumers in this fixture");
    sync_ok(&backend1, events).await;

    // ... then a moment later, independently to server2 (newer). A real
    // sleep, not a mocked clock: server2's write must land at a genuinely
    // later wall-clock timestamp than server1's for `find_latest` to be
    // able to tell them apart by `X-Last-Modified` — which is APISIX's own
    // second-granularity header, not our millisecond `stable_timestamp`, so
    // the gap has to clear a full second (matches the TS reference's own
    // `wait(1000)` in this same scenario) or the two can tie and
    // `find_latest`'s tie-break (whichever probe's response completes
    // last, not whichever was actually written last) picks arbitrarily.
    tokio::time::sleep(Duration::from_millis(1100)).await;

    common::cache().invalidate(cache_key).await;
    let backend2 = backend_for(common::SERVER2, cache_key);
    let events = diff(&service_with_route_config(3306), &empty_configuration());
    sync_ok(&backend2, events).await;

    // server3 was never written at all — a real 3-way race between an
    // untouched, an older, and a newer instance.
    common::cache().invalidate(cache_key).await;
    let backend_multi = backend_multi(cache_key);
    let config = dump(&backend_multi).await;

    if common::apisix_version() > semver::Version::new(3, 13, 0) {
        let services = config.services.expect("the winning server has data");
        let port = services[0].upstream.as_ref().unwrap().nodes.as_ref().unwrap()[0].port;
        assert_eq!(port, 3306, "must pick server2's (the more recently written) document, not server1's");
    } else {
        assert!(common::cache().versions(cache_key).await.is_some());
    }
}

#[tokio::test]
#[ignore]
async fn bypass_cache_discards_stale_state_and_refetches() {
    common::restart_apisix().await;
    let cache_key = "cache-e2e-bypass";
    common::cache().invalidate(cache_key).await;

    let backend = backend(cache_key);
    dump(&backend).await;
    let synced = fixture_config(9180);
    sync_ok(&backend, diff(&synced, &empty_configuration())).await;
    let cached = common::cache().config(cache_key).await.unwrap();
    assert_eq!(cached.services.unwrap()[0].name, "service1");

    // Inject data that doesn't exist on the real server, simulating a
    // cache that's gone stale relative to it.
    let mut stale = fixture_config(80);
    stale.services.as_mut().unwrap()[0].name = "stale-service".to_string();
    common::cache().set_config(cache_key, stale.clone()).await;
    assert_eq!(common::cache().config(cache_key).await.unwrap().services.unwrap()[0].name, "stale-service");

    // Without bypassing, dump serves the (now stale) cache as-is.
    let result = dump(&backend).await;
    assert_eq!(result.services.unwrap()[0].name, "stale-service");

    // A backend with `bypass_cache` discards it and re-fetches for real.
    let mut opts = common::backend_options(vec![common::SERVER1.to_string()], cache_key);
    opts.bypass_cache = true;
    let bypass_backend = Backend::new(opts).unwrap();

    let result = dump(&bypass_backend).await;
    assert_eq!(result.services.as_ref().unwrap()[0].name, "service1", "bypassing must re-fetch the real server state, not the stale cache");

    // The cache is now repopulated with the fresh data ...
    let cached = common::cache().config(cache_key).await.unwrap();
    assert_eq!(cached.services.as_ref().unwrap()[0].name, "service1");

    // ... and a subsequent non-bypassing dump serves that repopulated cache.
    let result = dump(&backend).await;
    assert_eq!(result.services.unwrap()[0].name, "service1");
}
