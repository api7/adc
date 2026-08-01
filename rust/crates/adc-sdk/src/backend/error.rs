use std::error::Error as StdError;

/// Failure modes shared by every `Backend` implementation. The TS codebase had
/// no equivalent taxonomy — call sites threw bare `Error`/`AxiosError` values
/// or stuffed an `Error` into a result struct's `error?` field — so this is a
/// new design surface, not a port.
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
