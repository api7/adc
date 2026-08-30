//! No TS reference spec to port from — this is a from-scratch addition.
//! Every other test touching `X-Digest` (the header `operator.rs::sha1_hex`
//! computes to let APISIX skip reprocessing an unchanged document) only
//! checks its *effect*: that resyncing unchanged state doesn't move any
//! `modifiedIndex`/`conf_version`. None of them ever look at the actual PUT
//! response to confirm APISIX itself recognized the digest and returned
//! `204`, rather than accepting and reprocessing an identical body every
//! time — a distinct guarantee: if the digest ever stopped matching (a
//! future serialization change, say), the value-level tests would keep
//! passing while this optimization silently stopped firing. Real network
//! calls against a live 3-instance standalone APISIX cluster — see
//! `common`'s module doc for how to bring one up and run this file.

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

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// PUTs the *exact* document `common::raw_config()` just read straight back,
/// with a caller-chosen digest header, bypassing `Backend`/`Operator`
/// entirely — this crate's own `Operator::sync` always computes a digest
/// that's correct *by construction* for whatever it just built, so the only
/// way to observe APISIX's response to a *wrong* digest is to send one by
/// hand. `ApisixStandalone`'s fields are all plain, unflattened struct
/// fields with no map ordering ambiguity above one already-sorted
/// `BTreeMap`-backed label map, so re-serializing what `raw_config()`
/// deserialized reproduces byte-for-byte what a real no-op resync would
/// have PUT — this isn't approximating the mechanism, it's the same bytes.
async fn put_raw(body: &str, digest: &str) -> reqwest::StatusCode {
    let client = HttpClient::new(HttpClientConfig {
        server: common::SERVER1.to_string(),
        token: common::TOKEN.to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let request = client.request(Method::PUT, "/apisix/admin/configs").unwrap().header(HEADER_DIGEST, digest).body(body.to_string());
    client.send(request).await.unwrap().status()
}

#[tokio::test]
#[ignore]
async fn a_resync_with_the_correct_digest_gets_a_204_a_wrong_one_does_not() {
    common::restart_apisix().await;
    let backend = backend("digest-e2e");
    dump(&backend).await;

    let before = dump(&backend).await;
    let local = Configuration {
        services: Some(vec![adc::Service {
            name: "svc1".to_string(),
            upstream: Some(adc::Upstream {
                nodes: Some(vec![adc::UpstreamNode { host: "127.0.0.1".to_string(), port: 9180, weight: 100, priority: 0, metadata: None }]),
                ..base_upstream()
            }),
            ..base_service()
        }]),
        ..empty_configuration()
    };
    sync_ok(&backend, diff(&local, &before)).await;

    let raw = common::raw_config().await;
    let body = serde_json::to_string(&raw).unwrap();
    let correct_digest = sha1_hex(body.as_bytes());

    // The document is genuinely unchanged from what's already stored — the
    // correct digest for it must get APISIX's own no-op short-circuit.
    let status = put_raw(&body, &correct_digest).await;
    assert_eq!(status, reqwest::StatusCode::NO_CONTENT, "a PUT whose digest matches the currently stored document must get a 204");

    // The exact same body, but a digest that doesn't match anything real —
    // APISIX must not skip processing just because the *content* happens
    // to be identical to what it already has; the whole mechanism is
    // digest-driven, not a content diff.
    let status = put_raw(&body, "0000000000000000000000000000000000000000").await;
    assert_ne!(status, reqwest::StatusCode::NO_CONTENT, "a PUT with a wrong digest must not get the no-op 204, even with identical content");

    // Confirm that second PUT didn't corrupt anything — the document must
    // still read back exactly the same afterward.
    assert_eq!(common::raw_config().await, raw);
}
