//! Reproduces the actual multi-writer conflict (see `impl/standalone/
//! p01-multi-writer.md`): another writer already advanced a collection's
//! `*_conf_version` past what this crate's next `sync()` computes, and
//! APISIX rejects the PUT with `400`. The exact message this asserts on
//! was captured verbatim from a real 3.17.0 instance (seed an inflated
//! `services_conf_version` via raw PUT, then PUT a normal one back) —
//! `operator.rs`'s own unit tests cover `conf_version_rejection_message`'s
//! classification logic in isolation; this only needs to prove a real
//! rejection still propagates to the caller unchanged, the one thing that
//! can't be checked without an actual gateway. Real network calls against
//! a live 3-instance standalone APISIX cluster — see `common`'s module doc
//! for how to bring one up and run this file.

use adc_backend_apisix_standalone::tests::typing::ApisixStandalone;
use adc_backend_apisix_standalone::Backend;
use adc_backend_core::{HttpClient, HttpClientConfig, Method, TlsConfig};
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;
use sha1::{Digest, Sha1};

mod common;
use common::{backend, base_service, base_upstream, diff, empty_configuration};

const HEADER_DIGEST: &str = "x-digest";

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// PUTs `raw` back with its `services_conf_version` inflated far past
/// anything a real sync would ever compute — bypasses `Backend`/`Operator`
/// entirely, the same way `e2e_digest_short_circuit.rs`'s `put_raw` does,
/// since this crate's own `Operator::sync` only ever PUTs a document whose
/// versions it just computed correctly, never a stale one.
async fn seed_inflated_services_conf_version(mut raw: ApisixStandalone) {
    // Comfortably below 2^53 (~9.007e15) — APISIX's Lua/cjson round-trips
    // numbers as doubles, so anything past that precision loses digits and
    // comes back as scientific notation (`4.6116860184274e+18` for
    // `i64::MAX / 2`, observed firsthand), which our `i64` field then
    // fails to deserialize.
    raw.services_conf_version = 99_999_999_999_999;
    let body = serde_json::to_string(&raw).unwrap();
    let digest = sha1_hex(body.as_bytes());
    let client = HttpClient::new(HttpClientConfig {
        server: common::SERVER1.to_string(),
        token: common::TOKEN.to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let request = client.request(Method::PUT, "/apisix/admin/configs").unwrap().header(HEADER_DIGEST, digest).body(body);
    let status = client.send(request).await.unwrap().status();
    assert!(status.is_success(), "seeding the inflated conf_version must itself succeed: got {status}");
}

#[tokio::test]
#[ignore]
async fn a_stale_conf_version_rejection_propagates_to_the_caller_unchanged() {
    common::restart_apisix().await;
    let cache_key = "conf-version-conflict-e2e";
    let backend = backend(cache_key);

    let before = dump(&backend).await;
    let raw = common::raw_config().await;
    seed_inflated_services_conf_version(raw).await;

    // This crate's own cache still thinks `services_conf_version` is
    // whatever `before` saw — a normal sync computes a normal (small)
    // timestamp for it, which the server now rejects: someone else (this
    // test, standing in for another writer) already moved it far ahead.
    let local = Configuration {
        services: Some(vec![adc::Service {
            name: "svc1".to_string(),
            upstream: Some(adc::Upstream { nodes: Some(vec![adc::UpstreamNode { host: "127.0.0.1".to_string(), port: 9180, weight: 100, priority: 0, metadata: None }]), ..base_upstream() }),
            ..base_service()
        }]),
        ..empty_configuration()
    };
    let events = diff(&local, &before);
    assert!(!events.is_empty());

    let error = backend
        .sync(events, BackendSyncOptions::default())
        .await
        .expect_err("a stale conf_version must still surface as an error to the caller, unchanged");
    let message = error.to_string();
    assert!(message.contains("conf_version"), "the real APISIX rejection message must reach the caller verbatim, got: {message}");

    // The failed attempt must not have perturbed the server's actual state.
    let after = common::raw_config().await;
    assert_eq!(after.services_conf_version, 99_999_999_999_999);
    assert_eq!(after.services.len(), 0, "the rejected write must not have landed");
}
