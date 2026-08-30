//! No TS reference spec to port from — this is a from-scratch addition.
//! `Cache`'s own unit tests (in `src/cache.rs`) prove the raw per-key mutex
//! is race-safe in isolation; this proves the *composed* guarantee actually
//! holds when real, independent `Backend` instances (mirroring independent
//! concurrent requests to the ingress-server, which builds a fresh `Backend`
//! per request rather than sharing one) race `sync()` on the same
//! `cache_key` through the real pipeline (`adc_differ::apply` reconstruction,
//! `ChangeSet`, `stamp_versions`, the cache read-modify-write). Real network
//! calls against a live 3-instance standalone APISIX cluster — see
//! `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::Backend as _;
use adc_sdk::{BackendSyncOptions, ResourceType};
use serde_json::json;

mod common;
use common::{backend, create_event};

async fn dump(backend: &Backend) -> adc_sdk::resources::Configuration {
    backend.dump().await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore]
async fn concurrent_syncs_on_the_same_cache_key_never_lose_an_update() {
    common::restart_apisix().await;
    let cache_key = "concurrent-sync-e2e";
    dump(&backend(cache_key)).await;

    const N: usize = 8;
    let mut handles = Vec::with_capacity(N);
    for i in 0..N {
        let cache_key = cache_key.to_string();
        handles.push(tokio::spawn(async move {
            // A fresh `Backend` per task, same as `adc-cli`'s server layer
            // building one per incoming request — the shared state that
            // actually has to serialize this race is `Cache::global()`, not
            // anything held on one `Backend` value. Consumers (unlike
            // global rules) aren't validated against a fixed plugin
            // registry, so arbitrary distinct usernames are accepted.
            let backend = backend(&cache_key);
            let name = format!("concurrent-user-{i}");
            let event = create_event(ResourceType::Consumer, &name, json!({ "username": name }), None);
            backend.sync(vec![event], BackendSyncOptions::default()).await
        }));
    }

    for handle in handles {
        let results = handle.await.expect("task must not panic").expect("sync must not error");
        for result in &results {
            assert!(result.success, "{:?}: {:?}", result.server, result.error);
        }
    }

    let config = dump(&backend(cache_key)).await;
    let consumers = config.consumers.expect("all concurrent creates must have landed");
    for i in 0..N {
        let username = format!("concurrent-user-{i}");
        assert!(consumers.iter().any(|c| c.username == username), "{username} must not have been lost to a lost-update race");
    }

    // Every one of the N racing syncs stamped *some* timestamp; the
    // collection's own conf_version must equal the highest one actually
    // used, not an earlier one a later writer's read-modify-write dropped.
    let raw = common::raw_config().await;
    let max_index = raw.consumers.iter().map(|c| c.modified_index()).max().unwrap();
    assert_eq!(raw.consumers_conf_version, max_index, "conf_version must reflect the actual latest write, not a stale one");

    // And every stamped modifiedIndex must be a distinct value — if two
    // concurrent writers' read-modify-write windows overlapped incorrectly,
    // two consumers could end up sharing (or worse, one overwriting
    // another's) timestamp in a way that isn't just "they happened to land
    // in the same batch" (a legitimate single sync can share one timestamp
    // across many resources — these N are N *separate* sync calls, so a
    // collision here would mean the serialization broke, not that a batch
    // was atomic).
    let mut indices: Vec<i64> = raw.consumers.iter().map(|c| c.modified_index()).collect();
    indices.sort_unstable();
    indices.dedup();
    assert_eq!(indices.len(), N, "each of the N separate sync calls must have stamped a distinct timestamp");
}
