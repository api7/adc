//! JSON structured logging for the ingress-server daemon. Level is
//! controlled by `ADC_INGRESS_LOG_LEVEL` (default `info`), not `--verbose`.

use axum::body::Body;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

pub fn init() {
    let level = std::env::var("ADC_INGRESS_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let env_filter =
        tracing_subscriber::EnvFilter::try_new(&level).unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().json().with_env_filter(env_filter).init();
}

pub async fn request_logger(request: Request, next: Next) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    tracing::info!(request_id = %request_id, "{method} {path}");

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, 100 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(request_id = %request_id, %error, "failed to read request body");
            let status = if std::error::Error::source(&error).is_some_and(|source| source.is::<http_body_util::LengthLimitError>()) {
                axum::http::StatusCode::PAYLOAD_TOO_LARGE
            } else {
                axum::http::StatusCode::BAD_REQUEST
            };
            return status.into_response();
        }
    };
    if !bytes.is_empty() {
        tracing::debug!(request_id = %request_id, request_body = %redacted_body_text(&bytes));
    }

    let request = Request::from_parts(parts, Body::from(bytes));
    next.run(request).await
}

fn redacted_body_text(bytes: &[u8]) -> String {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => redact_request_body(&value).to_string(),
        // Non-JSON bytes could still contain a raw token/key substring —
        // logging them verbatim would defeat the redaction above.
        Err(_) => format!("<non-JSON body, {} bytes>", bytes.len()),
    }
}

/// Never let a raw mTLS private key or backend token reach a debug log.
pub fn redact_request_body(body: &Value) -> Value {
    let mut redacted = body.clone();
    for pointer in ["/task/opts/tlsClientKey", "/task/opts/token"] {
        if let Some(field) = redacted.pointer_mut(pointer) {
            *field = Value::String("***".to_string());
        }
    }
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_tls_client_key_while_preserving_other_fields() {
        let body = json!({"task": {"opts": {"backend": "apisix", "tlsClientKey": "SECRET", "tlsClientCert": "cert"}, "config": {}}});
        let redacted = redact_request_body(&body);
        assert_eq!(
            redacted,
            json!({"task": {"opts": {"backend": "apisix", "tlsClientKey": "***", "tlsClientCert": "cert"}, "config": {}}})
        );
    }

    #[test]
    fn redacts_the_backend_token() {
        let body = json!({"task": {"opts": {"backend": "apisix", "token": "SECRET"}, "config": {}}});
        let redacted = redact_request_body(&body);
        assert_eq!(
            redacted,
            json!({"task": {"opts": {"backend": "apisix", "token": "***"}, "config": {}}})
        );
    }

    #[test]
    fn returns_the_body_unchanged_when_tls_client_key_is_absent() {
        let body = json!({"task": {"opts": {"backend": "apisix"}, "config": {}}});
        assert_eq!(redact_request_body(&body), body);
    }

    #[test]
    fn does_not_panic_on_malformed_bodies() {
        for body in [
            json!({"task": {"opts": 1}}),
            json!({"task": {"opts": "not-an-object"}}),
            json!({"task": {"opts": null}}),
            json!({"task": {}}),
            json!({}),
            Value::Null,
        ] {
            let _ = redact_request_body(&body);
        }
    }
}
