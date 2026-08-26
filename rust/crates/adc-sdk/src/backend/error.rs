use std::error::Error as StdError;

/// Failure modes shared by every `Backend` implementation, so callers can
/// match on a closed set of variants instead of an opaque error type.
///
/// Concrete backends (apisix, api7, apisix-standalone) map their own
/// transport/serialization errors into these variants; anything that doesn't
/// fit a specific variant goes through `Other`.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("network request failed: {0}")]
    Transport(String),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("resource not found: {0}")]
    NotFound(String),

    /// The backend service reached us and responded, but rejected the
    /// request or reported failure at the application level (as opposed to a
    /// transport-level failure, which is `Transport`).
    #[error("backend rejected the request (status {status}): {message}")]
    Api { status: u16, message: String },

    #[error("failed to (de)serialize backend payload: {0}")]
    Serialization(String),

    #[error("operation not supported by this backend: {0}")]
    Unsupported(String),

    #[error(transparent)]
    Other(#[from] Box<dyn StdError + Send + Sync>),
}

impl BackendError {
    /// Whether retrying the same request again could plausibly succeed,
    /// judging only by what every backend has in common. `Transport`
    /// (timeout, connection refused, ...) and a 5xx `Api` response are — the
    /// backend or network had a transient problem, not an objection to the
    /// request itself. A 4xx `Api` response (bad payload, unsupported
    /// config, ...) won't change on a retry: the backend already looked at
    /// the request and rejected it.
    ///
    /// A concrete backend may know its own additional retriable cases (an
    /// APISIX dependency-ordering conflict, say) that don't fit this
    /// general rule — this is a floor for retry policies to build on, not
    /// the full classification for every backend.
    pub fn is_retriable(&self) -> bool {
        match self {
            BackendError::Transport(_) => true,
            BackendError::Api { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_5xx_api_error_is_retriable() {
        let err = BackendError::Api { status: 502, message: "bad gateway".into() };
        assert!(err.is_retriable());
    }

    #[test]
    fn a_plain_4xx_api_error_is_not_retriable() {
        let err = BackendError::Api { status: 400, message: "bad config".into() };
        assert!(!err.is_retriable());
    }

    #[test]
    fn transport_errors_are_always_retriable() {
        assert!(BackendError::Transport("connection refused".into()).is_retriable());
    }
}
