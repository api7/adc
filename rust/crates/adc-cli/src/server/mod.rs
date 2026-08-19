//! The ingress-server daemon: a long-running HTTP(S)/Unix-socket sidecar
//! exposing `PUT /sync`/`PUT /validate` on an adc listener,
//! `GET /healthz/ready` on a separate status listener.

pub mod agent_pool;
mod backend;
pub mod logging;
mod schema;
mod sync;
mod validate;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::Value;
use tokio::sync::watch;

use crate::cli::IngressServerArgs;
use crate::error::CliError;

const MAX_BODY_BYTES: usize = 100 * 1024 * 1024;

/// Bound on how long the HTTPS listener waits for in-flight (e.g.
/// keep-alive) connections to finish once a shutdown starts — without this,
/// an idle keep-alive connection could block process exit indefinitely.
const HTTPS_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(10);

/// Checks the current value first so an already-sent `true` isn't missed.
async fn wait_for_shutdown(mut rx: watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    let _ = rx.changed().await;
}

fn adc_router() -> Router {
    Router::new()
        .route("/sync", put(sync::sync_handler))
        .route("/validate", put(validate::validate_handler))
        .layer(axum::middleware::from_fn(logging::request_logger))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
}

fn status_router(ready: Arc<AtomicBool>) -> Router {
    Router::new()
        .route("/healthz/ready", get(healthz))
        .with_state(ready)
}

pub async fn run(args: IngressServerArgs) -> Result<(), CliError> {
    // `signal()` registers synchronously, unlike `ctrl_c()` (an async fn
    // that only registers on first poll) — called first to close the race
    // where an early SIGINT hits before a spawned ctrl_c task gets polled.
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|e| CliError::msg(format!("failed to install SIGINT handler: {e}")))?;

    let app = adc_router();

    let ready = Arc::new(AtomicBool::new(false));
    let status_app = status_router(ready.clone());

    let status_addr = SocketAddr::from((IpAddr::V4(Ipv4Addr::UNSPECIFIED), args.listen_status));
    let status_listener = tokio::net::TcpListener::bind(status_addr)
        .await
        .map_err(|e| {
            CliError::msg(format!(
                "failed to bind status listener on {status_addr}: {e}"
            ))
        })?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let status_shutdown = shutdown_rx.clone();
    let status_server = async move {
        axum::serve(status_listener, status_app)
            .with_graceful_shutdown(wait_for_shutdown(status_shutdown))
            .await
            .map_err(|e| CliError::msg(format!("status server error: {e}")))
    };

    tracing::info!(
        "ADC server is running on: {}",
        display_listen_address(&args.listen)
    );
    let adc_server = serve_adc(&args, app, shutdown_rx, ready);

    tokio::spawn(async move {
        sigint.recv().await;
        tracing::info!("Stopping, see you next time!");
        let _ = shutdown_tx.send(true);
    });

    tokio::try_join!(adc_server, status_server)?;
    Ok(())
}

async fn healthz(State(ready): State<Arc<AtomicBool>>) -> Response {
    if ready.load(Ordering::Acquire) {
        (StatusCode::OK, "ok").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready").into_response()
    }
}

async fn serve_adc(
    args: &IngressServerArgs,
    app: Router,
    shutdown: watch::Receiver<bool>,
    ready: Arc<AtomicBool>,
) -> Result<(), CliError> {
    match args.listen.scheme() {
        "unix" => serve_unix(args, app, shutdown, ready).await,
        "https" => serve_https(args, app, shutdown, ready).await,
        _ => serve_http(args, app, shutdown, ready).await,
    }
}

async fn serve_http(
    args: &IngressServerArgs,
    app: Router,
    shutdown: watch::Receiver<bool>,
    ready: Arc<AtomicBool>,
) -> Result<(), CliError> {
    let addr = tcp_addr(&args.listen)?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| CliError::msg(format!("failed to bind {addr}: {e}")))?;
    ready.store(true, Ordering::Release);
    axum::serve(listener, app)
        .with_graceful_shutdown(wait_for_shutdown(shutdown))
        .await
        .map_err(|e| CliError::msg(format!("server error: {e}")))
}

/// An existing stale socket file is removed before binding, and the fresh
/// one gets `0o660` permissions once bound. The socket file itself is
/// removed again on shutdown so a normal restart doesn't depend on this
/// stale-file cleanup happening next time.
async fn serve_unix(
    args: &IngressServerArgs,
    app: Router,
    shutdown: watch::Receiver<bool>,
    ready: Arc<AtomicBool>,
) -> Result<(), CliError> {
    let path = args.listen.path();
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path)
                .map_err(|e| CliError::msg(format!("failed to remove stale socket {path}: {e}")))?;
        }
        Ok(_) => {
            return Err(CliError::msg(format!(
                "refusing to bind unix socket: {path} already exists and is not a socket"
            )));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(CliError::msg(format!("failed to inspect {path}: {e}"))),
    }
    let listener = tokio::net::UnixListener::bind(path)
        .map_err(|e| CliError::msg(format!("failed to bind unix socket {path}: {e}")))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))
        .map_err(|e| CliError::msg(format!("failed to chmod unix socket {path}: {e}")))?;
    ready.store(true, Ordering::Release);
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(wait_for_shutdown(shutdown))
        .await
        .map_err(|e| CliError::msg(format!("server error: {e}")));
    std::fs::remove_file(path).ok();
    result
}

async fn serve_https(
    args: &IngressServerArgs,
    app: Router,
    shutdown: watch::Receiver<bool>,
    ready: Arc<AtomicBool>,
) -> Result<(), CliError> {
    let cert_path = args
        .tls_cert_file
        .as_deref()
        .ok_or_else(|| CliError::msg("--tls-cert-file is required when --listen uses https"))?;
    let key_path = args
        .tls_key_file
        .as_deref()
        .ok_or_else(|| CliError::msg("--tls-key-file is required when --listen uses https"))?;
    let server_config = build_rustls_config(cert_path, key_path, args.ca_cert_file.as_deref())?;
    let addr = tcp_addr(&args.listen)?;

    let handle = axum_server::Handle::new();
    let handle_for_shutdown = handle.clone();
    tokio::spawn(async move {
        wait_for_shutdown(shutdown).await;
        handle_for_shutdown.graceful_shutdown(Some(HTTPS_SHUTDOWN_DEADLINE));
    });

    let handle_for_ready = handle.clone();
    tokio::spawn(async move {
        if handle_for_ready.listening().await.is_some() {
            ready.store(true, Ordering::Release);
        }
    });

    let tls_config = axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(server_config));
    axum_server::tls_rustls::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .map_err(|e| CliError::msg(format!("server error: {e}")))
}

/// `requestCert: true, rejectUnauthorized: true` when a CA is given (mTLS);
/// plain server-only TLS otherwise.
fn build_rustls_config(
    cert_path: &Path,
    key_path: &Path,
    ca_path: Option<&Path>,
) -> Result<rustls::ServerConfig, CliError> {
    crate::install_crypto_provider();

    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;

    let builder = rustls::ServerConfig::builder();
    let builder = match ca_path {
        Some(ca_path) => {
            let mut roots = rustls::RootCertStore::empty();
            for cert in load_certs(ca_path)? {
                roots
                    .add(cert)
                    .map_err(|e| CliError::msg(format!("invalid CA certificate: {e}")))?;
            }
            let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots))
                .build()
                .map_err(|e| CliError::msg(format!("invalid CA certificate: {e}")))?;
            builder.with_client_cert_verifier(verifier)
        }
        None => builder.with_no_client_auth(),
    };
    builder
        .with_single_cert(certs, key)
        .map_err(|e| CliError::msg(format!("invalid TLS certificate/key: {e}")))
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, CliError> {
    let file =
        std::fs::File::open(path).map_err(|e| CliError::msg(format!("{}: {e}", path.display())))?;
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>, CliError> {
    let file =
        std::fs::File::open(path).map_err(|e| CliError::msg(format!("{}: {e}", path.display())))?;
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))?
        .ok_or_else(|| CliError::msg(format!("{}: no private key found", path.display())))
}

fn tcp_addr(listen: &url::Url) -> Result<SocketAddr, CliError> {
    let host = listen
        .host_str()
        .ok_or_else(|| CliError::msg("--listen must include a host"))?;
    let port = listen
        .port_or_known_default()
        .ok_or_else(|| CliError::msg("--listen must include a port"))?;
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| CliError::msg(format!("could not resolve listen address {host}:{port}")))
}

fn display_listen_address(listen: &url::Url) -> String {
    if listen.scheme() == "unix" {
        listen.path().to_string()
    } else {
        listen.as_str().trim_end_matches('/').to_string()
    }
}

fn bad_request(body: Value) -> Response {
    (StatusCode::BAD_REQUEST, Json(body)).into_response()
}

fn internal_error(body: Value) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use super::*;

    fn tls_asset(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/tls"))
            .join(name)
    }

    async fn send(router: Router, method: &str, path: &str, body: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn healthz_ready_returns_ok_once_marked_ready() {
        let router = status_router(Arc::new(AtomicBool::new(true)));
        let (status, _) = send(router, "GET", "/healthz/ready", "").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn healthz_ready_returns_service_unavailable_before_ready() {
        let router = status_router(Arc::new(AtomicBool::new(false)));
        let (status, _) = send(router, "GET", "/healthz/ready", "").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn sync_rejects_malformed_input_with_400() {
        let (status, body) = send(adc_router(), "PUT", "/sync", r#"{"not":"valid"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["message"].is_string(), "{body}");
    }

    #[tokio::test]
    async fn sync_rejects_unpaired_tls_client_material_and_names_the_field() {
        let body = serde_json::json!({
            "task": {
                "opts": {
                    "backend": "apisix", "server": "http://1.1.1.1:9180", "token": "t", "cacheKey": "default",
                    "tlsClientCert": "-----BEGIN CERTIFICATE-----\nx",
                },
                "config": {},
            }
        })
        .to_string();
        let (status, json) = send(adc_router(), "PUT", "/sync", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let errors = json["errors"].as_array().unwrap();
        assert!(
            errors
                .iter()
                .any(|e| e["path"] == serde_json::json!(["tlsClientKey"])),
            "{json}"
        );
    }

    #[tokio::test]
    async fn sync_against_an_unreachable_backend_returns_500() {
        let body = serde_json::json!({
            "task": {"opts": {"backend": "apisix", "server": "http://127.0.0.1:1", "token": "t", "cacheKey": "default"}, "config": {}}
        })
        .to_string();
        let (status, _) = send(adc_router(), "PUT", "/sync", &body).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn sync_rejects_a_configuration_that_fails_lint() {
        let body = serde_json::json!({
            "task": {
                "opts": {"backend": "apisix", "server": "http://1.1.1.1:9180", "token": "t", "cacheKey": "default"},
                "config": {"services": [{"name": ""}]},
            }
        })
        .to_string();
        let (status, json) = send(adc_router(), "PUT", "/sync", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["message"].as_str().unwrap().contains("Lint"), "{json}");
    }

    #[tokio::test]
    async fn validate_reports_source_input_on_a_malformed_body() {
        let (status, json) = send(adc_router(), "PUT", "/validate", "{}").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["source"], "input");
        assert_eq!(json["success"], false);
    }

    #[tokio::test]
    async fn validate_reports_source_lint_on_a_lint_failure() {
        let body = serde_json::json!({
            "task": {
                "opts": {"backend": "apisix", "server": "http://1.1.1.1:9180", "token": "t", "cacheKey": "default"},
                "config": {"services": [{"name": ""}]},
            }
        })
        .to_string();
        let (status, json) = send(adc_router(), "PUT", "/validate", &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["source"], "lint");
    }

    #[tokio::test]
    async fn http_listener_serves_real_requests_end_to_end() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server = tokio::spawn(async move {
            axum::serve(listener, adc_router())
                .with_graceful_shutdown(wait_for_shutdown(shutdown_rx))
                .await
                .unwrap();
        });

        let response = reqwest::Client::new()
            .put(format!("http://{addr}/sync"))
            .json(&serde_json::json!({"not": "valid"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

        let _ = shutdown_tx.send(true);
        server.await.unwrap();
    }

    /// A real `https://` listener requiring a client certificate.
    async fn spawn_https_mtls_listener(app: Router) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let config = build_rustls_config(
            &tls_asset("server.cer"),
            &tls_asset("server.key"),
            Some(&tls_asset("ca.cer")),
        )
        .unwrap();
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(config));
        let handle = axum_server::Handle::new();
        let server_handle = handle.clone();
        let task = tokio::spawn(async move {
            axum_server::tls_rustls::bind_rustls("127.0.0.1:0".parse().unwrap(), tls_config)
                .handle(server_handle)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });
        let addr = handle.listening().await.expect("server should have bound");
        (addr, task)
    }

    #[tokio::test]
    async fn https_listener_rejects_a_connection_without_a_client_certificate() {
        let (addr, task) = spawn_https_mtls_listener(adc_router()).await;
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let result = client
            .put(format!("https://{addr}/sync"))
            .body("{}")
            .send()
            .await;
        assert!(
            result.is_err(),
            "expected the TLS handshake to fail without a client certificate"
        );
        task.abort();
    }

    #[tokio::test]
    async fn https_listener_accepts_a_connection_with_a_valid_client_certificate() {
        let (addr, task) = spawn_https_mtls_listener(adc_router()).await;

        let mut identity_pem = std::fs::read_to_string(tls_asset("client.cer")).unwrap();
        identity_pem.push('\n');
        identity_pem.push_str(&std::fs::read_to_string(tls_asset("client.key")).unwrap());
        let identity = reqwest::Identity::from_pem(identity_pem.as_bytes()).unwrap();
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .identity(identity)
            .build()
            .unwrap();

        // A real HTTP response (not a TLS error) proves the handshake succeeded.
        let response = client
            .put(format!("https://{addr}/sync"))
            .header("content-type", "application/json")
            .body("{}")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        task.abort();
    }

    #[tokio::test]
    async fn unix_listener_removes_a_stale_socket_and_sets_0o660_permissions() {
        let dir = std::env::temp_dir().join(format!("adc-server-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("adc.sock");
        // A real leftover socket from a crashed previous run, not just any file.
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());

        let args = IngressServerArgs {
            listen: url::Url::parse(&format!("unix://{}", path.display())).unwrap(),
            listen_status: 0,
            ca_cert_file: None,
            tls_cert_file: None,
            tls_key_file: None,
        };
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let ready = Arc::new(AtomicBool::new(false));
        let server = tokio::spawn({
            let ready = ready.clone();
            async move { serve_unix(&args, adc_router(), shutdown_rx, ready).await }
        });

        // `serve_unix` only flips `ready` after `set_permissions` succeeds —
        // polling the socket's mere existence races the chmod that follows it.
        let mut became_ready = false;
        for _ in 0..100 {
            if ready.load(Ordering::Acquire) {
                became_ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(became_ready, "server never became ready");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o660);

        let _ = shutdown_tx.send(true);
        server.await.unwrap().unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_rustls_config_accepts_a_matching_cert_and_key_with_a_ca() {
        let config = build_rustls_config(
            &tls_asset("server.cer"),
            &tls_asset("server.key"),
            Some(&tls_asset("ca.cer")),
        );
        assert!(config.is_ok(), "{config:?}");
    }

    #[test]
    fn build_rustls_config_accepts_a_matching_cert_and_key_without_a_ca() {
        let config = build_rustls_config(&tls_asset("server.cer"), &tls_asset("server.key"), None);
        assert!(config.is_ok(), "{config:?}");
    }

    #[test]
    fn build_rustls_config_rejects_a_key_that_does_not_match_the_certificate() {
        let config = build_rustls_config(&tls_asset("server.cer"), &tls_asset("client.key"), None);
        assert!(config.is_err());
    }
}
