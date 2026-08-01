use std::time::Duration;

use adc_backend_core::{HttpClient, HttpClientConfig, Method, TlsConfig};
use adc_sdk::BackendError;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use axum::routing::get;
use serde_json::{Value, json};
use tokio::net::TcpListener;

async fn spawn_server() -> String {
    let router = Router::new()
        .route(
            "/apisix/admin/routes",
            get(
                |State(token): State<&'static str>, headers: HeaderMap| async move {
                    assert_eq!(headers.get("X-API-KEY").unwrap().to_str().unwrap(), token);
                    assert_eq!(
                        headers.get("Content-Type").unwrap().to_str().unwrap(),
                        "application/json"
                    );
                    Json(json!({ "list": [] }))
                },
            ),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Json(json!({}))
            }),
        )
        .with_state("test-token");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

fn client(server: String, timeout: Option<Duration>) -> HttpClient {
    HttpClient::new(HttpClientConfig {
        server,
        token: "test-token".into(),
        timeout,
        tls: TlsConfig::default(),
    })
    .unwrap()
}

#[tokio::test]
async fn injects_auth_header_and_decodes_success_response() {
    let server = spawn_server().await;
    let client = client(server, None);

    let req = client.request(Method::GET, "/apisix/admin/routes").unwrap();
    let resp = client.send(req).await.unwrap();
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body, json!({ "list": [] }));
}

#[tokio::test]
async fn classifies_404_as_not_found() {
    let server = spawn_server().await;
    let client = client(server, None);

    let req = client.request(Method::GET, "/does-not-exist").unwrap();
    let err = client.send(req).await.unwrap_err();
    assert!(
        matches!(err, BackendError::NotFound(_)),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn classifies_slow_response_as_transport_timeout() {
    let server = spawn_server().await;
    let client = client(server, Some(Duration::from_millis(50)));

    let req = client.request(Method::GET, "/slow").unwrap();
    let err = client.send(req).await.unwrap_err();
    let BackendError::Transport(message) = &err else {
        panic!("expected Transport, got {err:?}");
    };
    assert!(message.contains("timed out"), "message: {message}");
}
