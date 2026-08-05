//! Not a real-dashboard e2e test — a local, deliberately slow HTTP server
//! (same pattern as `adc-backend-core/tests/http_client.rs`) to check that
//! a timed-out request surfaces a message identifying which request it
//! was. Doesn't need `docker compose up`, so it isn't `#[ignore]`d.
//!
//! `sync` resolves only the gateway_group id before handing off to the
//! operator (`Operator` doesn't use a version or default-value at all), so
//! which request's timeout surfaces below is always deterministic:
//! `/api/gateway_groups` specifically.

use std::time::Duration;

use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::{Backend as _, BackendSyncOptions, Event, EventKind, ResourceType};
use axum::Router;
use axum::routing::any;
use serde_json::json;
use tokio::net::TcpListener;

/// Every path on this server hangs for far longer than any timeout these
/// tests configure, so every request reliably times out.
async fn spawn_slow_server() -> String {
    let router = Router::new().fallback(any(|| async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        axum::Json(json!({ "value": {} }))
    }));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

fn backend(server: &str, timeout: Duration) -> adc_backend_api7::Backend {
    let client = HttpClient::new(HttpClientConfig {
        server: server.to_string(),
        token: "test-token".to_string(),
        timeout: Some(timeout),
        tls: TlsConfig::default(),
    })
    .unwrap();
    adc_backend_api7::Backend::new(
        client,
        "default".to_string(),
        "test-token",
        adc_backend_core::ResourceFilter::default(),
    )
}

#[tokio::test]
async fn ping_timeout_names_the_request_that_timed_out() {
    let server = spawn_slow_server().await;
    let backend = backend(&server, Duration::from_millis(10));

    let error = backend.ping().await.unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(&format!("{server}/api/gateway_groups")),
        "{message}"
    );
    assert!(message.contains("timed out"), "{message}");
}

#[tokio::test]
async fn version_timeout_names_the_request_that_timed_out() {
    let server = spawn_slow_server().await;
    let backend = backend(&server, Duration::from_millis(10));

    let error = backend.version().await.unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(&format!("{server}/api/version")),
        "{message}"
    );
    assert!(message.contains("timed out"), "{message}");
}

#[tokio::test]
async fn dump_timeout_names_the_request_that_timed_out() {
    let server = spawn_slow_server().await;
    let backend = backend(&server, Duration::from_millis(10));

    // `dump` resolves the version before anything else, so this is the
    // request that actually times out first.
    let error = backend.dump().await.unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(&format!("{server}/api/version")),
        "{message}"
    );
    assert!(message.contains("timed out"), "{message}");
}

#[tokio::test]
async fn sync_timeout_names_the_request_that_timed_out() {
    let server = spawn_slow_server().await;
    let backend = backend(&server, Duration::from_millis(10));

    let event = Event::new(
        ResourceType::Consumer,
        EventKind::Create {
            new_value: json!({ "username": "test", "plugins": {} }),
        },
        "test-consumer",
        "test-consumer",
    );
    let error = backend
        .sync(
            vec![event],
            BackendSyncOptions {
                exit_on_failure: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains(&format!("{server}/api/gateway_groups")),
        "{message}"
    );
    assert!(message.contains("timed out"), "{message}");
}
