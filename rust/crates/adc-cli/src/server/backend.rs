//! Translates a request's `opts` into `pipeline::BackendSpec` and delegates
//! to `pipeline::init_backend` — the only server-specific pieces are
//! sourcing TLS material from inline PEM (not files) and getting the
//! `HttpClient` from the shared pool (see `agent_pool`) instead of building
//! a fresh one.

use adc_backend_core::{ResourceFilter, TlsConfig};
use adc_sdk::Backend;

use super::agent_pool::{self, TlsMaterial};
use super::schema::Opts;
use crate::error::CliError;
use crate::pipeline::{self, BackendSpec};

/// No fallible parts, unlike `BackendSpec`'s `TryFrom<&BackendArgs>`
/// (which rejects `managed-by` as a label selector) — nothing here can fail.
impl From<&Opts> for BackendSpec {
    fn from(opts: &Opts) -> Self {
        let (include, exclude) = opts.resource_type_sets();
        BackendSpec {
            kind: opts.backend.clone(),
            servers: opts.server.as_list(),
            tokens: opts.token.split(',').map(str::to_string).collect(),
            gateway_group: opts.gateway_group.clone(),
            filter: ResourceFilter {
                include,
                exclude,
                label_selector: opts.label_selector_or_default(),
            },
            concurrency: opts.request_concurrent,
            cache_key: opts.cache_key.clone(),
            bypass_cache: opts.bypass_cache,
            timeout: Some(std::time::Duration::from_millis(opts.timeout)),
            tls: TlsConfig {
                ca_cert_pem: opts.ca_cert.clone().map(String::into_bytes),
                client_cert_pem: opts.tls_client_cert.clone().map(String::into_bytes),
                client_key_pem: opts.tls_client_key.clone().map(String::into_bytes),
                skip_verify: opts.tls_skip_verify,
            },
        }
    }
}

pub fn build_backend(opts: &Opts) -> Result<Box<dyn Backend>, CliError> {
    let tls_material = TlsMaterial {
        skip_verify: opts.tls_skip_verify,
        ca_cert: opts.ca_cert.clone(),
        client_cert: opts.tls_client_cert.clone(),
        client_key: opts.tls_client_key.clone(),
    };
    let shared_client = agent_pool::get_client(&tls_material)?;
    pipeline::init_backend(opts.into(), Some((*shared_client).clone()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::schema::ServerAddr;
    use super::*;

    fn tls_asset(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/tls"))
            .join(name)
    }

    fn opts(server: &str, ca_cert: Option<String>) -> Opts {
        Opts {
            backend: "apisix".to_string(),
            server: ServerAddr::Single(server.to_string()),
            token: "token".to_string(),
            lint: true,
            include_resource_type: None,
            exclude_resource_type: None,
            label_selector: None,
            cache_key: "default".to_string(),
            bypass_cache: false,
            gateway_group: None,
            request_concurrent: 10,
            timeout: 30_000,
            tls_skip_verify: false,
            ca_cert,
            tls_client_cert: None,
            tls_client_key: None,
        }
    }

    /// A bare HTTPS stub answering every request with `200 {}`.
    async fn spawn_https_stub() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let cert = super::super::load_certs(&tls_asset("server.cer")).unwrap();
        let key = super::super::load_key(&tls_asset("server.key")).unwrap();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert, key)
            .unwrap();
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(config));

        let app = axum::Router::new().fallback(axum::routing::any(|| async {
            (axum::http::StatusCode::OK, "{}")
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let listener = listener.into_std().unwrap();
        let handle = axum_server::Handle::new();
        let server_handle = handle.clone();
        let task = tokio::spawn(async move {
            axum_server::from_tcp_rustls(listener, tls_config)
                .expect("failed to configure rustls acceptor")
                .handle(server_handle)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });
        handle.listening().await;
        (addr, task)
    }

    #[tokio::test]
    async fn ping_fails_certificate_verification_without_a_ca_cert() {
        let (addr, task) = spawn_https_stub().await;
        let backend = build_backend(&opts(&format!("https://{addr}"), None)).unwrap();
        let error = backend.ping().await.unwrap_err();
        assert!(
            matches!(error, adc_sdk::BackendError::Transport(_)),
            "expected a Transport error (certificate verification is a transport-level failure), got {error:?}"
        );
        assert!(
            error.to_string().to_lowercase().contains("certificate")
                || error.to_string().to_lowercase().contains("unknownissuer"),
            "{error}"
        );
        task.abort();
    }

    #[tokio::test]
    async fn ping_succeeds_once_the_signing_ca_is_provided() {
        let (addr, task) = spawn_https_stub().await;
        let ca = std::fs::read_to_string(tls_asset("ca.cer")).unwrap();
        let backend = build_backend(&opts(&format!("https://{addr}"), Some(ca))).unwrap();
        backend
            .ping()
            .await
            .expect("certificate verification should succeed with the correct CA");
        task.abort();
    }

    #[test]
    fn build_backend_constructs_an_api7ee_backend() {
        let mut o = opts("http://127.0.0.1:9180", None);
        o.backend = "api7ee".to_string();
        o.gateway_group = Some("prod".to_string());
        assert!(build_backend(&o).is_ok());
    }

    #[test]
    fn build_backend_constructs_an_apisix_standalone_backend_from_multiple_servers_and_tokens() {
        let mut o = opts("http://127.0.0.1:9180", None);
        o.backend = "apisix-standalone".to_string();
        o.server = ServerAddr::Multiple(vec![
            "http://127.0.0.1:9180".to_string(),
            "http://127.0.0.1:9181".to_string(),
        ]);
        o.token = "t1,t2".to_string();
        o.cache_key = "test-standalone-key".to_string();
        assert!(build_backend(&o).is_ok());
    }

    #[test]
    fn build_backend_rejects_an_apisix_standalone_backend_with_no_servers() {
        let mut o = opts("http://127.0.0.1:9180", None);
        o.backend = "apisix-standalone".to_string();
        o.server = ServerAddr::Multiple(vec![]);
        assert!(build_backend(&o).is_err());
    }
}
