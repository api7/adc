//! No TS reference spec to port from — this is a from-scratch addition.
//! `operator.rs`'s own unit tests already prove `ChangeSet::from_events`
//! surfaces a clear error for a malformed *input* (a missing `parent_id`);
//! this instead provokes a real rejection from a live server (wrong admin
//! token, unreachable address) and checks `dump`/`sync` surface a clear
//! `BackendError` — not a panic, not a silent empty result — and leave no
//! stale cache behind. Real network calls against a live 3-instance
//! standalone APISIX cluster — see `common`'s module doc for how to bring
//! one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::backend_options;

#[tokio::test]
#[ignore]
async fn dump_with_a_wrong_admin_token_errors_cleanly_and_leaves_no_cache() {
    common::restart_apisix().await;
    let cache_key = "wrong-token-dump-e2e";

    let mut opts = backend_options(vec![common::SERVER1.to_string()], cache_key);
    opts.tokens = vec!["definitely-not-the-real-token".to_string()];
    let backend = Backend::new(opts).unwrap();

    let result = backend.dump().await;
    assert!(result.is_err(), "dumping with a wrong admin token must surface an error, not succeed or panic");
    assert!(common::cache().config(cache_key).await.is_none(), "a failed dump must not leave a cache entry behind");
}

#[tokio::test]
#[ignore]
async fn sync_with_a_wrong_admin_token_errors_cleanly_and_resets_the_cache() {
    common::restart_apisix().await;
    let cache_key = "wrong-token-sync-e2e";

    let mut opts = backend_options(vec![common::SERVER1.to_string()], cache_key);
    opts.tokens = vec!["definitely-not-the-real-token".to_string()];
    let backend = Backend::new(opts).unwrap();

    let result = backend.sync(vec![], BackendSyncOptions::default()).await;
    assert!(result.is_err(), "syncing with a wrong admin token must surface an error, not succeed or panic");
    assert!(common::cache().config(cache_key).await.is_none(), "a failed sync must not leave a stale cache entry behind");
}

#[tokio::test]
#[ignore]
async fn dump_against_an_unreachable_server_errors_cleanly() {
    let cache_key = "unreachable-dump-e2e";
    let opts = backend_options(vec!["http://127.0.0.1:1".to_string()], cache_key);
    let backend = Backend::new(opts).unwrap();

    let result = backend.dump().await;
    assert!(result.is_err(), "dumping against an unreachable server must surface a connection error, not hang or panic");
}

#[tokio::test]
#[ignore]
async fn sync_against_an_unreachable_server_errors_cleanly() {
    let cache_key = "unreachable-sync-e2e";
    let opts = backend_options(vec!["http://127.0.0.1:1".to_string()], cache_key);
    let backend = Backend::new(opts).unwrap();

    let result = backend.sync(vec![], BackendSyncOptions::default()).await;
    assert!(result.is_err(), "syncing against an unreachable server must surface a connection error, not hang or panic");
    assert!(common::cache().config(cache_key).await.is_none());
}
