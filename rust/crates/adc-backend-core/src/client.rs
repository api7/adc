use std::time::Duration;

use adc_sdk::BackendError;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Method, RequestBuilder, Response, Url};

use crate::tls::TlsConfig;

/// Everything needed to stand up a backend's HTTP client: where it lives,
/// how to authenticate, and how to connect. Doesn't include backend-specific
/// concerns like a gateway group name — those live in each backend crate.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub server: String,
    pub token: String,
    pub timeout: Option<Duration>,
    pub tls: TlsConfig,
}

/// A backend's HTTP client: a `reqwest::Client` pre-configured with the
/// `X-API-KEY` auth header apisix/api7 both expect, TLS settings, and a base
/// URL, plus request/response handling that classifies failures into
/// `BackendError` uniformly. Connection pooling (the Node `agentkeepalive`
/// equivalent) comes for free from `reqwest::Client`'s own pool — nothing to
/// configure for that.
pub struct HttpClient {
    inner: reqwest::Client,
    base_url: Url,
}

impl HttpClient {
    pub fn new(config: HttpClientConfig) -> Result<Self, BackendError> {
        let base_url = Url::parse(&config.server)
            .map_err(|e| BackendError::Other(format!("invalid server URL {:?}: {e}", config.server).into()))?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let mut token = HeaderValue::from_str(&config.token)
            .map_err(|e| BackendError::Other(format!("invalid token: {e}").into()))?;
        token.set_sensitive(true);
        headers.insert("X-API-KEY", token);

        let mut builder = reqwest::Client::builder().default_headers(headers);
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        builder = config.tls.apply(builder)?;

        let inner = builder
            .build()
            .map_err(|e| BackendError::Other(format!("failed to build HTTP client: {e}").into()))?;

        Ok(Self { inner, base_url })
    }

    /// Starts a request against `path`, resolved relative to the configured
    /// server URL (e.g. `/apisix/admin/routes`).
    pub fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, BackendError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| BackendError::Other(format!("invalid request path {path:?}: {e}").into()))?;
        Ok(self.inner.request(method, url))
    }

    /// Sends a request built via [`HttpClient::request`], classifying
    /// transport failures and non-2xx responses into `BackendError`. Callers
    /// decode the body themselves (`.json()`, `.text()`, or just read
    /// headers) since the right shape depends on the endpoint.
    pub async fn send(&self, builder: RequestBuilder) -> Result<Response, BackendError> {
        let (client, request) = builder.build_split();
        let request = request.map_err(|e| BackendError::Other(format!("failed to build request: {e}").into()))?;
        let method = request.method().clone();
        let url = request.url().clone();

        let response = client.execute(request).await.map_err(|e| classify_transport_error(&e, &method, &url))?;

        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let message = response.text().await.unwrap_or_default();
        Err(match status.as_u16() {
            401 | 403 => BackendError::Auth(message),
            404 => BackendError::NotFound(format!("{method} {url}")),
            _ => BackendError::Api { status: status.as_u16(), message },
        })
    }
}

fn classify_transport_error(error: &reqwest::Error, method: &Method, url: &Url) -> BackendError {
    if error.is_timeout() {
        BackendError::Transport(format!(
            "request \"{method} {url}\" timed out. Consider increasing the timeout."
        ))
    } else {
        BackendError::Transport(format!("request \"{method} {url}\" failed: {error}"))
    }
}
