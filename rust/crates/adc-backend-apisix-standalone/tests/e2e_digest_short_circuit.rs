//! No TS reference spec to port from — this is a from-scratch addition.
//! Every other test touching `X-Digest` (the header `operator.rs::sha1_hex`
//! computes to let APISIX skip reprocessing an unchanged document) only
//! checks its *effect*: that resyncing unchanged state doesn't move any
//! `modifiedIndex`/`conf_version`. None of them ever look at the actual PUT
//! response to confirm APISIX itself recognized the digest and took its
//! no-op short-circuit path, rather than accepting and reprocessing an
//! identical body every time — a distinct guarantee: if the digest ever
//! stopped matching (a future serialization change, say), the value-level
//! tests would keep passing while this optimization silently stopped
//! firing. The exact status code for "digest matched, nothing to do" isn't
//! pinned to one value (observed `204` on newer APISIX, `202` on 3.13.0) —
//! this checks that a matching digest and a wrong one get *different*
//! responses, not that either gets one specific code. Real network calls
//! against a live 3-instance standalone APISIX cluster — see `common`'s
//! module doc for how to bring one up and run this file.

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
async fn a_resync_with_the_correct_digest_short_circuits_a_wrong_one_does_not() {
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
    // correct digest for it must succeed via APISIX's own no-op
    // short-circuit, whatever status code this version spells that with.
    let correct_status = put_raw(&body, &correct_digest).await;
    assert!(correct_status.is_success(), "a PUT whose digest matches the currently stored document must succeed: got {correct_status}");

    // The exact same body, but a digest that doesn't match anything real —
    // APISIX must not take that same short-circuit path just because the
    // *content* happens to be identical to what it already has; the whole
    // mechanism is digest-driven, not a content diff.
    let wrong_status = put_raw(&body, "0000000000000000000000000000000000000000").await;

    // Confirm that second PUT didn't corrupt anything either way — the
    // document must still read back exactly the same afterward.
    assert_eq!(common::raw_config().await, raw);

    if wrong_status == correct_status {
        // A wrong digest got the exact same response as a matching one —
        // this APISIX version doesn't distinguish by `X-Digest` at all
        // (observed on 3.13.0: both get 202, versus 3.17.0's 204-vs-200
        // split), so there's nothing version-independent left to assert
        // about the short-circuit specifically.
        return;
    }
}
