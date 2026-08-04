//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! (the same stack the TS `backend-api7` e2e suite uses) — dashboard at
//! `https://localhost:7443` (self-signed cert). `common::client()`/
//! `common::token()` handle the login/password-rotation/license-activation
//! /token-minting dance a fresh dashboard needs themselves (see
//! `common`'s module doc), so nothing beyond the running dashboard is
//! required; set `SERVER`/`GATEWAY_GROUP`/`TOKEN`/`BACKEND_API7_LICENSE`/
//! `BACKEND_API7_VERSION` to override the defaults the TS e2e suite's own
//! `global-setup.ts` uses.
//!
//! Ignored by default (`cargo test` never touches the network); run with
//! `cargo test -p adc-backend-api7 --test e2e_gateway_group -- --ignored --test-threads=1`.

use adc_backend_api7::tests::GatewayGroupResolver;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};

mod common;
use common::{client, gateway_group, token};

#[tokio::test]
#[ignore]
async fn resolves_the_configured_gateway_group_to_a_real_id() {
    let resolver = GatewayGroupResolver::new(client().await, gateway_group(), &token().await);

    let id = resolver.resolve().await.unwrap();
    assert!(id.is_some(), "expected a resolved gateway group id");

    // Second call must come from the cache, not a fresh lookup — the
    // resolved id is stable for the resolver's lifetime.
    assert_eq!(resolver.resolve().await.unwrap(), id);
}

#[tokio::test]
#[ignore]
async fn errors_when_the_configured_gateway_group_does_not_exist() {
    let name = "adc-rust-e2e-does-not-exist";
    let resolver = GatewayGroupResolver::new(client().await, name.to_string(), &token().await);

    let error = resolver.resolve().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&format!("Gateway group \"{name}\" does not exist")),
        "unexpected error message: {error}"
    );
}

/// No dashboard needed: an `a7adm-` prefixed token short-circuits before
/// any request is made, so this exercises real (if unreachable) client
/// behavior rather than standing in for API7's own responses — not
/// gated behind `--ignored`.
#[tokio::test]
async fn an_admin_token_skips_resolution_without_making_any_request() {
    let client = HttpClient::new(HttpClientConfig {
        server: "http://127.0.0.1:1".to_string(),
        token: "a7adm-test".to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let resolver = GatewayGroupResolver::new(client, "prod".to_string(), "a7adm-test");
    assert_eq!(resolver.resolve().await.unwrap(), None);
}
