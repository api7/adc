//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! (the same stack the TS `backend-api7` e2e suite uses) — see
//! `tests/common/mod.rs`'s module doc for how a dashboard gets bootstrapped
//! into a usable state.
//!
//! Ignored by default (`cargo test` never touches the network); run with
//! `cargo test -p adc-backend-api7 --test e2e_ping -- --ignored --test-threads=1`.

use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::Backend as _;

mod common;

#[tokio::test]
#[ignore]
async fn ping_succeeds_against_a_real_dashboard() {
    let backend = common::backend().await;
    backend.ping().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn ping_fails_against_an_unreachable_server() {
    // `127.0.0.1:1`, not bare `0.0.0.0`: on Linux, connecting to `0.0.0.0`
    // gets silently remapped to loopback by the kernel, so if port 80
    // happens to have something listening (plausible on a CI runner) this
    // test would connect instead of failing the way it's meant to. Port 1
    // on loopback is about as reliably unbound as it gets.
    let client = HttpClient::new(HttpClientConfig {
        server: "http://127.0.0.1:1".to_string(),
        token: String::new(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let backend = adc_backend_api7::Backend::new(
        client,
        common::gateway_group(),
        "",
        adc_backend_core::ResourceFilter::default(),
    );

    let error = backend.ping().await.unwrap_err();
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("refused") || message.to_lowercase().contains("connect"),
        "unexpected error message: {message}"
    );
}

#[tokio::test]
#[ignore]
async fn ping_fails_against_a_self_signed_certificate_without_skip_verify() {
    // Same server as `common::backend()`, but without `skip_verify` — the
    // dashboard's self-signed cert must be rejected.
    let client = HttpClient::new(HttpClientConfig {
        server: common::server(),
        token: common::token().await,
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let backend = adc_backend_api7::Backend::new(
        client,
        common::gateway_group(),
        &common::token().await,
        adc_backend_core::ResourceFilter::default(),
    );

    let error = backend.ping().await.unwrap_err();
    let message = error.to_string().to_lowercase();
    assert!(
        message.contains("certificate") || message.contains("cert"),
        "unexpected error message: {message}"
    );
}
