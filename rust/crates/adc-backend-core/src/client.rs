use std::time::Duration;

use adc_sdk::BackendError;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Method, RequestBuilder, Response, Url};

use crate::tls::TlsConfig;

/// RFC 3986's "unreserved" characters left unescaped, matching how path
/// segments are conventionally percent-encoded (e.g. `encodeURIComponent`);
/// everything else, including `/`, gets encoded.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

/// Percent-encodes one URL path segment, for backends building an
/// admin-API path from a user-controlled id (a `Consumer.username`, an
/// explicit resource `id`, ...) — without this, such a value could inject
/// a `/`, a query string, or a fragment into the request `Url::parse`
/// eventually builds from it. `PATH_SEGMENT` leaves `.` unescaped (it's an
/// RFC 3986 unreserved character), so a segment that's *exactly* `.` or
/// `..` is rejected outright here instead: `url`'s parser normalizes
/// dot-segments in *any* path it parses, not just during relative
/// reference resolution, so an unescaped `.`/`..` segment can still
/// traverse to a different admin-API path than the one requested even
/// though [`HttpClient::request`] itself no longer resolves paths via
/// `Url::join`.
pub fn encode_path_segment(segment: &str) -> Result<String, BackendError> {
    if segment == "." || segment == ".." {
        return Err(BackendError::Other(format!("{segment:?} is not a valid resource id").into()));
    }
    Ok(utf8_percent_encode(segment, PATH_SEGMENT).to_string())
}

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
/// `X-API-KEY` auth header APISIX/API7 both expect, TLS settings, and a base
/// URL, plus request/response handling that classifies failures into
/// `BackendError` uniformly. Connection pooling (the Node `agentkeepalive`
/// equivalent) comes for free from `reqwest::Client`'s own pool — nothing to
/// configure for that.
///
/// Cheap to clone: `reqwest::Client` is internally `Arc`-backed and shares
/// its connection pool across clones, so handing each of a backend's
/// fetcher/operator/validator its own owned `HttpClient` (rather than a
/// borrow with a lifetime to thread through) costs nothing beyond a couple
/// of atomic increments.
#[derive(Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    base_url: Url,
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

        let mut builder = reqwest::Client::builder().default_headers(headers);
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        builder = config.tls.apply(builder)?;

        let inner = builder.build().map_err(|e| {
            BackendError::Other(format!("failed to build HTTP client: {}", with_source(&e)).into())
        })?;

        Ok(Self { inner, base_url })
    }

    /// Starts a request against `path` (e.g. `/apisix/admin/routes`),
    /// appended onto the configured server URL. Plain string concatenation
    /// (trimming exactly one `/` at the join point) rather than
    /// `Url::join`: a root-anchored `path` handed to `Url::join` replaces
    /// the base URL's path outright per RFC 3986, which would silently
    /// drop any path prefix the server URL carries (e.g. APISIX's admin
    /// API exposed behind a reverse-proxy prefix like
    /// `https://host/gateway/`) — matching the TS backend's own
    /// axios-based client, which combines `baseURL` and a request path the
    /// same simple way.
    pub fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, BackendError> {
        let base = self.base_url.as_str().trim_end_matches('/');
        let path = path.trim_start_matches('/');
        let combined = format!("{base}/{path}");
        let url = Url::parse(&combined).map_err(|e| {
            BackendError::Other(format!("invalid request path {path:?}: {e}").into())
        })?;
        Ok(self.inner.request(method, url))
    }

    /// Sends a request built via [`HttpClient::request`], classifying only
    /// transport-level failures (timeout, connection refused, ...) into
    /// `BackendError::Transport`. Unlike [`HttpClient::send`], any response
    /// that actually comes back — including a non-2xx one — is returned as
    /// `Ok`, for the handful of endpoints where a 404 (or worse) is a
    /// meaningful result rather than a failure (e.g. probing whether a
    /// resource or an admin-API path exists at all on this backend version).
    pub async fn execute(&self, builder: RequestBuilder) -> Result<Response, BackendError> {
        let (client, request) = builder.build_split();
        let request = request
            .map_err(|e| BackendError::Other(format!("failed to build request: {e}").into()))?;
        let method = request.method().clone();
        let url = request.url().clone();

        client
            .execute(request)
            .await
            .map_err(|e| classify_transport_error(&e, &method, &url))
    }

    /// Sends a request built via [`HttpClient::request`], classifying
    /// transport failures and non-2xx responses into `BackendError`. Callers
    /// decode the body themselves (`.json()`, `.text()`, or just read
    /// headers) since the right shape depends on the endpoint.
    pub async fn send(&self, builder: RequestBuilder) -> Result<Response, BackendError> {
        let response = self.execute(builder).await?;
        Self::require_success(response).await
    }

    /// Applies [`HttpClient::send`]'s non-2xx-to-`BackendError` mapping to a
    /// response obtained via [`HttpClient::execute`], for callers that need
    /// to inspect the status themselves before deciding whether to treat it
    /// as an error.
    pub async fn require_success(response: Response) -> Result<Response, BackendError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let url = response.url().clone();
        let message = response.text().await.unwrap_or_default();
        Err(match status.as_u16() {
            401 | 403 => BackendError::Auth(message),
            404 => BackendError::NotFound(url.to_string()),
            _ => BackendError::Api {
                status: status.as_u16(),
                message,
            },
        })
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

/// `reqwest::Error`'s own `Display` is often terse ("builder error",
/// "error sending request") with the actually useful detail (a TLS
/// validation failure's specific reason, say) only available via
/// `.source()`. This walks the chain and appends it.
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
        assert_eq!(encode_path_segment("my-consumer_1.local~x").unwrap(), "my-consumer_1.local~x");
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
}
