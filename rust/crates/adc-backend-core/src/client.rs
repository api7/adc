use std::time::Duration;

use adc_sdk::BackendError;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, HeaderMap, HeaderName, HeaderValue, SET_COOKIE};
use reqwest::{Method, RequestBuilder, Response, ResponseBuilderExt, Url};
use serde::de::DeserializeOwned;
use tracing::Instrument;

use crate::tls::TlsConfig;

pub const HTTP_REQUEST_SPAN_NAME: &str = "http_request";

/// RFC 3986 unreserved characters, matching `encodeURIComponent`-style
/// path-segment escaping.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Percent-encodes a user-controlled path segment (a `Consumer.username`,
/// a resource id) so it can't inject a `/`, query, or fragment. `.`/`..`
/// segments are rejected outright — `Url`'s parser normalizes dot-segments
/// in any path, so an unescaped one could still traverse elsewhere.
pub fn encode_path_segment(segment: &str) -> Result<String, BackendError> {
    if segment == "." || segment == ".." {
        return Err(BackendError::Other(
            format!("{segment:?} is not a valid resource id").into(),
        ));
    }
    Ok(utf8_percent_encode(segment, PATH_SEGMENT).to_string())
}

/// Everything needed to stand up a backend's HTTP client.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub server: String,
    pub token: String,
    pub timeout: Option<Duration>,
    pub tls: TlsConfig,
}

/// A backend's HTTP client: `X-API-KEY` auth, TLS, and a base URL, plus
/// `BackendError`-uniform failure classification. Cheap to clone —
/// `reqwest::Client` is `Arc`-backed and shares its connection pool.
#[derive(Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    base_url: Url,
    /// `reqwest::Client` only merges `default_headers` at send time — a
    /// built `Request` never carries them — so `execute`'s debug span keeps
    /// its own copy to show the complete header set.
    default_headers: HeaderMap,
    /// Tags `execute`'s debug span with the owning backend's log scope
    /// (`"APISIX"`), not the generic `"ADC"`. Set via `with_log_scope`
    /// since `HttpClient` itself doesn't know which backend owns it.
    log_scope: Vec<String>,
}

impl HttpClient {
    pub fn new(config: HttpClientConfig) -> Result<Self, BackendError> {
        let base_url = Url::parse(&config.server).map_err(|e| {
            BackendError::Other(format!("invalid server URL {:?}: {e}", config.server).into())
        })?;

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let mut token = HeaderValue::from_str(&config.token)
            .map_err(|e| BackendError::Other(format!("invalid token: {e}").into()))?;
        token.set_sensitive(true);
        headers.insert("X-API-KEY", token);
        let default_headers = headers.clone();

        let mut builder = reqwest::Client::builder().default_headers(headers);
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        builder = config.tls.apply(builder)?;

        let inner = builder.build().map_err(|e| {
            BackendError::Other(format!("failed to build HTTP client: {}", with_source(&e)).into())
        })?;

        Ok(Self {
            inner,
            base_url,
            default_headers,
            log_scope: vec!["ADC".to_string()],
        })
    }

    /// Called by the backend crate wrapping this client (e.g.
    /// `adc_backend_apisix::Backend::new`) with its own `metadata().log_scope`.
    pub fn with_log_scope(mut self, log_scope: Vec<String>) -> Self {
        self.log_scope = log_scope;
        self
    }

    /// Plain string concatenation, not `Url::join` — a root-anchored `path`
    /// via `join` would replace the base URL's path outright, dropping any
    /// prefix the server URL carries (e.g. behind a reverse-proxy).
    pub fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, BackendError> {
        let base = self.base_url.as_str().trim_end_matches('/');
        let path = path.trim_start_matches('/');
        let combined = format!("{base}/{path}");
        let url = Url::parse(&combined).map_err(|e| {
            BackendError::Other(format!("invalid request path {path:?}: {e}").into())
        })?;
        Ok(self.inner.request(method, url))
    }

    /// Classifies only transport-level failures into `BackendError::Transport`.
    /// Unlike [`HttpClient::send`], a non-2xx response is still `Ok` — some
    /// callers need to see a 404 as a meaningful result, not a failure.
    pub async fn execute(&self, builder: RequestBuilder) -> Result<Response, BackendError> {
        self.execute_described(builder, "").await
    }

    /// [`HttpClient::execute`] plus a one-line description shown in
    /// `--verbose 2`'s debug block, for requests whose purpose isn't
    /// obvious from the URL alone.
    pub async fn execute_described(
        &self,
        builder: RequestBuilder,
        description: &str,
    ) -> Result<Response, BackendError> {
        let (client, request) = builder.build_split();
        let request = request
            .map_err(|e| BackendError::Other(format!("failed to build request: {e}").into()))?;
        let method = request.method().clone();
        let url = request.url().clone();
        let server_address = url.host_str().unwrap_or("").to_string();
        let server_port = url.port_or_known_default().unwrap_or(0);

        // `request.headers()` alone misses `default_headers` (merged only
        // at send time); request-set headers win on conflict.
        let mut request_headers = self.default_headers.clone();
        for (name, value) in request.headers() {
            request_headers.insert(name.clone(), value.clone());
        }

        let span = tracing::debug_span!(
            HTTP_REQUEST_SPAN_NAME,
            http.request.method = %method,
            url.full = %redacted_url(&url),
            server.address = %server_address,
            server.port = server_port,
            scope = %self.log_scope.join("/"),
            description = %description,
            request_headers = %format_headers(&request_headers),
            request_body = tracing::field::Empty,
            http.response.status_code = tracing::field::Empty,
            network.protocol.version = tracing::field::Empty,
            response_headers = tracing::field::Empty,
            response_body = tracing::field::Empty,
            error.type = tracing::field::Empty,
            error_message = tracing::field::Empty,
        );
        // `is_disabled` is false only at `--verbose 2` — skips buffering
        // the response body off the stream otherwise.
        let disabled = span.is_disabled();
        if !disabled && let Some(body) = request.body().and_then(|b| b.as_bytes()) {
            span.record("request_body", String::from_utf8_lossy(body).as_ref());
        }
        let recording_span = span.clone();

        async move {
            match client.execute(request).await {
                Ok(response) if !disabled => {
                    let status = response.status();
                    let version = response.version();
                    let response_url = response.url().clone();
                    let headers = response.headers().clone();
                    let body = match response.bytes().await {
                        Ok(body) => body,
                        Err(e) => {
                            recording_span.record("error.type", transport_error_type(&e));
                            let error = classify_transport_error(&e, &method, &url);
                            recording_span.record("error_message", error.to_string());
                            return Err(error);
                        }
                    };
                    recording_span.record("http.response.status_code", status.as_u16() as i64);
                    recording_span.record("network.protocol.version", protocol_version(version));
                    recording_span.record("response_headers", format_headers(&headers));
                    recording_span.record("response_body", String::from_utf8_lossy(&body).as_ref());

                    // Rebuild a Response from the buffered bytes so the
                    // caller can still `.json()`/`.text()` normally — `url`
                    // and `version` need to be carried over explicitly, or
                    // `Response::from` falls back to a placeholder URL and
                    // HTTP/1.1, breaking callers that read `response.url()`
                    // (e.g. `require_success`'s 404 message).
                    let mut rebuilt = http::Response::builder()
                        .status(status)
                        .version(version)
                        .url(response_url);
                    for (name, value) in headers.iter() {
                        rebuilt = rebuilt.header(name, value);
                    }
                    let http_response = rebuilt
                        .body(body)
                        .expect("status/headers came from a real response; rebuilding can't fail");
                    Ok(Response::from(http_response))
                }
                Ok(response) => Ok(response),
                Err(e) => {
                    recording_span.record("error.type", transport_error_type(&e));
                    let error = classify_transport_error(&e, &method, &url);
                    recording_span.record("error_message", error.to_string());
                    Err(error)
                }
            }
        }
        .instrument(span)
        .await
    }

    /// Classifies transport failures *and* non-2xx responses into
    /// `BackendError`. Callers decode the body themselves.
    pub async fn send(&self, builder: RequestBuilder) -> Result<Response, BackendError> {
        let response = self.execute(builder).await?;
        Self::require_success(response).await
    }

    /// [`HttpClient::send`]'s non-2xx mapping, for callers that need to
    /// inspect the status themselves first.
    pub async fn require_success(response: Response) -> Result<Response, BackendError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let url = response.url().clone();
        let body = response.text().await.unwrap_or_default();
        let message = extract_error_message(&body);
        Err(match status.as_u16() {
            401 | 403 => BackendError::Auth(message),
            404 => BackendError::NotFound(url.to_string()),
            _ => BackendError::Api {
                status: status.as_u16(),
                message,
            },
        })
    }

    /// [`HttpClient::send`] plus JSON-decoding the body as `T` — the
    /// "request, require success, parse" sequence most admin-API calls
    /// otherwise repeat by hand. Callers with their own status handling
    /// before deciding whether/how to parse (a 404 that means "absent" ---
    /// versus "error", a status-keyed response shape) decode the `Response`
    /// themselves instead — their control flow genuinely branches earlier
    /// than this covers.
    pub async fn send_json<T: DeserializeOwned>(
        &self,
        builder: RequestBuilder,
    ) -> Result<T, BackendError> {
        let response = self.send(builder).await?;
        let url = response.url().clone();
        response
            .json()
            .await
            .map_err(|e| BackendError::Serialization(format!("decoding response from {url}: {e}")))
    }
}

/// Best-effort unwrap of APISIX's `{"error_msg": "..."}` error body down to
/// just the message, matching the TS CLI's `formatAxiosErrorMessage`. Falls
/// back to the raw body when it isn't that shape.
fn extract_error_message(body: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(serde_json::Value::Object(map)) => match map.get("error_msg") {
            Some(serde_json::Value::String(msg)) => msg.clone(),
            _ => body.to_string(),
        },
        _ => body.to_string(),
    }
}

/// `Header-Name: value\n`-per-line, matching the TS CLI's `transformHeaders`.
/// Sensitive values print as `*****` — either because the crate that set
/// them marked them so (`X-API-KEY`, via `HeaderValue::set_sensitive`), or
/// because the name itself is always credential-bearing regardless of who
/// set it (`is_sensitive_header_name`).
fn format_headers(headers: &HeaderMap) -> String {
    headers
        .iter()
        .map(|(name, value)| {
            let value = if value.is_sensitive() || is_sensitive_header_name(name) {
                "*****".to_string()
            } else {
                String::from_utf8_lossy(value.as_bytes()).into_owned()
            };
            format!("{name}: {value}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_sensitive_header_name(name: &HeaderName) -> bool {
    name == AUTHORIZATION || name == COOKIE || name == SET_COOKIE
}

/// OTel's `url.full` requires credentials to be redacted. Our own auth
/// never goes through the URL, but this is cheap defense anyway.
fn redacted_url(url: &Url) -> String {
    if url.username().is_empty() && url.password().is_none() {
        return url.to_string();
    }
    let mut redacted = url.clone();
    let _ = redacted.set_username("REDACTED");
    let _ = redacted.set_password(Some("REDACTED"));
    redacted.to_string()
}

/// OTel expects "1.1"/"2"/"3", not `Debug`'s `HTTP/1.1`.
fn protocol_version(version: reqwest::Version) -> &'static str {
    match version {
        reqwest::Version::HTTP_09 => "0.9",
        reqwest::Version::HTTP_10 => "1.0",
        reqwest::Version::HTTP_11 => "1.1",
        reqwest::Version::HTTP_2 => "2",
        reqwest::Version::HTTP_3 => "3",
        _ => "unknown",
    }
}

/// Low-cardinality `error.type` classifier — `error_message` carries the
/// full text.
fn transport_error_type(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_body() {
        "body"
    } else if error.is_decode() {
        "decode"
    } else if error.is_request() {
        "request"
    } else {
        "other"
    }
}

fn classify_transport_error(error: &reqwest::Error, method: &Method, url: &Url) -> BackendError {
    if error.is_timeout() {
        BackendError::Transport(format!(
            "request \"{method} {url}\" timed out. Consider increasing the timeout."
        ))
    } else {
        BackendError::Transport(format!(
            "request \"{method} {url}\" failed: {}",
            with_source(error)
        ))
    }
}

/// `reqwest::Error`'s `Display` is often terse; the useful detail is in
/// `.source()`. Walks the chain and appends it.
fn with_source(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_unreserved_characters_alone() {
        assert_eq!(
            encode_path_segment("my-consumer_1.local~x").unwrap(),
            "my-consumer_1.local~x"
        );
    }

    #[test]
    fn encodes_a_path_separator_so_it_cant_split_the_url() {
        assert_eq!(encode_path_segment("a/b").unwrap(), "a%2Fb");
    }

    #[test]
    fn encodes_query_and_fragment_delimiters() {
        assert_eq!(encode_path_segment("a?b=c#d").unwrap(), "a%3Fb%3Dc%23d");
    }

    #[test]
    fn rejects_a_single_dot_segment() {
        assert!(encode_path_segment(".").is_err());
    }

    #[test]
    fn rejects_a_double_dot_segment() {
        assert!(encode_path_segment("..").is_err());
    }

    #[test]
    fn a_dot_elsewhere_in_the_segment_is_fine() {
        assert_eq!(encode_path_segment("..a").unwrap(), "..a");
        assert_eq!(encode_path_segment("a..").unwrap(), "a..");
    }

    #[test]
    fn unwraps_apisix_error_msg_from_the_response_body() {
        let body = r#"{"error_msg":"property \"count\" validation failed"}"#;
        assert_eq!(
            extract_error_message(body),
            "property \"count\" validation failed"
        );
    }

    #[test]
    fn falls_back_to_the_raw_body_without_an_error_msg_field() {
        assert_eq!(
            extract_error_message(r#"{"foo":"bar"}"#),
            r#"{"foo":"bar"}"#
        );
        assert_eq!(extract_error_message("not json"), "not json");
        assert_eq!(extract_error_message(""), "");
    }

    #[test]
    fn redacts_credential_bearing_headers_by_name_even_when_not_marked_sensitive() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert(COOKIE, HeaderValue::from_static("session=secret"));
        headers.insert(SET_COOKIE, HeaderValue::from_static("session=secret"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let formatted = format_headers(&headers);
        assert!(formatted.contains("authorization: *****"));
        assert!(formatted.contains("cookie: *****"));
        assert!(formatted.contains("set-cookie: *****"));
        assert!(formatted.contains("content-type: application/json"));
    }
}
