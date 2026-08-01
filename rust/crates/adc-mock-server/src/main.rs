//! A minimal fake APISIX Admin API server, functionally equivalent to
//! `apps/cli/bench/mock-apisix-standalone-server.mjs` (a plain Node HTTP
//! server) but running on tokio's default multi-thread runtime — one worker
//! thread per available core — instead of Node's single JS thread. It does
//! not implement real gateway semantics: every GET list endpoint returns
//! empty, every PUT/DELETE/HEAD returns success immediately. Exists to let a
//! benchmark client be measured against a server that can itself use
//! multiple cores, to check whether a single-threaded server was capping the
//! numbers.
//!
//! Usage: adc-mock-server [port]   (default 18899, matching the Node
//! version's default, so the two are interchangeable in benchmark scripts.)

use axum::{
    Router,
    body::Bytes,
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
};

async fn handler(method: Method, uri: Uri, body: Bytes) -> Response {
    let path = uri.path();

    match method {
        Method::HEAD => (StatusCode::OK, "").into_response(),
        Method::GET => {
            let body = if path.starts_with("/apisix/admin/configs") {
                "{}".to_string()
            } else {
                r#"{"list":[],"total":0}"#.to_string()
            };
            ([("content-type", "application/json")], body).into_response()
        }
        Method::PUT | Method::POST => {
            let value = if body.is_empty() {
                "null".to_string()
            } else {
                String::from_utf8_lossy(&body).into_owned()
            };
            let payload = format!(r#"{{"key":"{path}","value":{value}}}"#);
            ([("content-type", "application/json")], payload).into_response()
        }
        Method::DELETE => {
            let payload = format!(r#"{{"deleted":"{path}"}}"#);
            ([("content-type", "application/json")], payload).into_response()
        }
        _ => (StatusCode::NOT_FOUND, "{}").into_response(),
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(18899);

    let app = Router::new().fallback(any(handler));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.expect("bind");

    println!(
        "mock apisix admin api (rust/tokio, {} worker threads available) listening on 127.0.0.1:{port}",
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    );

    axum::serve(listener, app).await.expect("serve");
}
