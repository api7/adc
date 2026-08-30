//! No TS reference spec to port from — this is a from-scratch addition.
//! `backend.rs`'s `sync` has explicit, commented recovery logic for a
//! multi-server write where only some servers accept the PUT ("a server
//! earlier in the batch may have already accepted the new document before a
//! later one failed") — this exercises that logic against a real partial
//! failure instead of only reasoning about it in a comment. Real network
//! calls against a live 3-instance standalone APISIX cluster, plus one
//! address nothing listens on to force a real, fast connection failure —
//! see `common`'s module doc for how to bring the cluster up and run this
//! file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::{backend_multi, backend_options, base_service, base_upstream, diff, empty_configuration};

const UNREACHABLE: &str = "http://127.0.0.1:1";

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

fn service_with_node(name: &str, port: u16) -> adc::Service {
    adc::Service { name: name.to_string(), upstream: Some(adc::Upstream { nodes: Some(vec![node(port)]), ..base_upstream() }), ..base_service() }
}

fn node(port: u16) -> adc::UpstreamNode {
    adc::UpstreamNode { host: "127.0.0.1".to_string(), port, weight: 100, priority: 0, metadata: None }
}

fn config_with_services(services: Vec<adc::Service>) -> Configuration {
    Configuration { services: Some(services), ..empty_configuration() }
}

#[tokio::test]
#[ignore]
async fn a_partial_multi_server_failure_is_reported_and_still_caches_the_servers_that_succeeded() {
    common::restart_apisix().await;
    let cache_key = "partial-failure-e2e";

    // Seed via a fully-healthy 3-real-server backend first, so `flaky`
    // below can `dump()` from the warm cache without ever needing to probe
    // its own unreachable server.
    let healthy = backend_multi(cache_key);
    dump(&healthy).await;

    let flaky = Backend::new(backend_options(vec![common::SERVER1.to_string(), common::SERVER2.to_string(), UNREACHABLE.to_string()], cache_key)).unwrap();

    let before = dump(&flaky).await;
    let local = config_with_services(vec![service_with_node("svc1", 9180)]);
    let events = diff(&local, &before);
    let opts = BackendSyncOptions { exit_on_failure: Some(false), ..Default::default() };
    let results = flaky.sync(events, opts).await.unwrap();

    let successes = results.iter().filter(|r| r.success).count();
    let failures = results.iter().filter(|r| !r.success).count();
    assert_eq!(successes, 2, "the two real servers must succeed");
    assert_eq!(failures, 1, "the unreachable server must be reported as a failure, not silently dropped");
    assert!(results.iter().any(|r| !r.success && r.error.is_some()), "the failed result must carry a real error, not just success:false");

    // The real servers actually got the write — read SERVER1 directly,
    // bypassing this crate's own cache.
    let raw = common::raw_config().await;
    assert_eq!(raw.services.len(), 1);
    assert_eq!(raw.services[0].name, "svc1");

    // "At least one server accepted the write" still caches the new
    // state — verified against what the *successful* servers now hold, not
    // a document the unreachable one never received.
    let cached = common::cache().config(cache_key).await.expect("a partial success (any server accepted) must still cache the new state");
    assert_eq!(cached.services.unwrap()[0].name, "svc1");
}

#[tokio::test]
#[ignore]
async fn exit_on_failure_true_aborts_and_resets_the_cache_entry_for_that_key() {
    common::restart_apisix().await;
    let cache_key = "partial-failure-abort-e2e";

    let healthy = backend_multi(cache_key);
    dump(&healthy).await;

    let flaky = Backend::new(backend_options(vec![common::SERVER1.to_string(), common::SERVER2.to_string(), UNREACHABLE.to_string()], cache_key)).unwrap();

    let before = dump(&flaky).await;
    let local = config_with_services(vec![service_with_node("svc1", 9180)]);
    let events = diff(&local, &before);
    let opts = BackendSyncOptions { exit_on_failure: Some(true), ..Default::default() };
    let result = flaky.sync(events, opts).await;

    assert!(result.is_err(), "exit_on_failure=true must surface the unreachable server's failure as an error, not silently succeed");
    assert!(
        common::cache().config(cache_key).await.is_none(),
        "a failed sync with exit_on_failure=true must reset the cache entry, so a later dump() re-fetches instead of trusting it"
    );
}
