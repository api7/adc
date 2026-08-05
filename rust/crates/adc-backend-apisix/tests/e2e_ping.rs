//! Ported from `libs/backend-apisix/e2e/ping.e2e-spec.ts`. Real network
//! calls against a live APISIX (plain HTTP on :19180, mTLS on :29180) — see
//! `e2e_apisix.rs`'s module doc for how to bring one up and run this file.
//!
//! Exact error text isn't asserted where TS checks for one: Node's TLS
//! stack (OpenSSL) and Rust's (rustls) report connection/certificate
//! failures in their own wording — what's portable is *that* the call
//! fails and, where meaningful, which `BackendError` variant it fails as.

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
// Trait-only import: brings `.ping()`/`.dump()`/etc. into scope without
// binding the name `Backend`, which would collide with the APISIX crate's
// own `Backend` type (its concrete implementation of this trait).
use adc_sdk::Backend as _;

mod common;
use common::TOKEN;

fn read_asset(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../libs/backend-apisix/e2e/assets/apisix_conf/mtls")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn backend(server: &str, tls: TlsConfig) -> ApisixBackend {
    let client = HttpClient::new(HttpClientConfig {
        server: server.to_string(),
        token: TOKEN.to_string(),
        timeout: None,
        tls,
    })
    .unwrap();
    ApisixBackend::new(client, adc_backend_core::ResourceFilter::default())
}

#[tokio::test]
#[ignore]
async fn succeeds_over_plain_http() {
    let backend = backend("http://localhost:19180", TlsConfig::default());
    backend.ping().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn succeeds_over_mtls() {
    let tls = TlsConfig {
        ca_cert_pem: Some(read_asset("ca.cer")),
        client_cert_pem: Some(read_asset("client.cer")),
        client_key_pem: Some(read_asset("client.key")),
        skip_verify: false,
    };
    let backend = backend("https://localhost:29180", tls);
    backend.ping().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn fails_against_an_unreachable_server() {
    let backend = backend("http://0.0.0.0:1", TlsConfig::default());
    let err = backend.ping().await.unwrap_err();
    assert!(
        matches!(err, adc_sdk::BackendError::Transport(_)),
        "got {err:?}"
    );
}

#[tokio::test]
#[ignore]
async fn fails_when_the_server_certificate_is_not_trusted() {
    // mTLS endpoint's cert is self-signed for this test fixture; not
    // supplying the CA to trust it must fail the TLS handshake.
    let backend = backend("https://localhost:29180", TlsConfig::default());
    let err = backend.ping().await.unwrap_err();
    assert!(
        matches!(err, adc_sdk::BackendError::Transport(_)),
        "got {err:?}"
    );
}

#[tokio::test]
#[ignore]
async fn fails_when_the_client_certificate_is_missing() {
    let tls = TlsConfig {
        ca_cert_pem: Some(read_asset("ca.cer")),
        client_cert_pem: None,
        client_key_pem: None,
        skip_verify: false,
    };
    let backend = backend("https://localhost:29180", tls);
    // APISIX's mTLS listener requires a client cert; without one the TLS
    // handshake itself is refused before any HTTP response comes back.
    assert!(backend.ping().await.is_err());
}
