//! The `Backend` trait: the interface every gateway integration (APISIX,
//! api7, apisix-standalone) implements, and the shared result/error types
//! that flow across it. `adc-sdk` only defines the contract — concrete
//! implementations live in their own crates and depend on this one.

mod error;

pub use error::BackendError;

use async_trait::async_trait;
use semver::Version;

use crate::{DefaultValue, Event, resources::Configuration};

/// The `tracing` span name a `Backend::sync` implementation should wrap
/// each individual event's application in — a real span (entered for that
/// event's whole lifetime, closed when it's done), not a synthetic
/// start/finish pair faked out of two plain events. This is the contract
/// callers (the CLI's progress display, and eventually any
/// `tracing-opentelemetry` export) rely on to find "one synced event" in
/// the trace: match on this exact span name, don't rely on a private
/// convention re-typed at each call site. Fields are left to each
/// implementation to declare (this crate doesn't prescribe a schema for
/// them), but should be plain data — no pre-formatted display text.
pub const SYNC_EVENT_SPAN_NAME: &str = "sync_event";

/// Static, non-behavioral facts about a `Backend` implementation, used by the
/// CLI to scope log output (e.g. `[APISIX]`) without the trait needing a
/// `name()`-shaped method per concern.
#[derive(Debug, Clone, Default)]
pub struct BackendMetadata {
    pub log_scope: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BackendSyncOptions {
    pub concurrent: Option<usize>,
    pub exit_on_failure: Option<bool>,
}

#[derive(Debug)]
pub struct BackendSyncResult {
    pub success: bool,
    pub event: Event,
    pub error: Option<BackendError>,
    pub server: Option<String>,
}

/// `resource_type` stays a raw string, not `ResourceType`: it's whatever the
/// backend's own validation response echoes back, which isn't guaranteed to
/// map onto one of our known resource types (an unrecognized value must
/// degrade gracefully — carry the string through, leave `event`/
/// `resource_name` unset — rather than fail the whole validate call).
#[derive(Debug, Clone)]
pub struct BackendValidationError {
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub index: usize,
    pub error: String,
    pub event: Option<Event>,
}

#[derive(Debug, Clone, Default)]
pub struct BackendValidateResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub errors: Vec<BackendValidationError>,
}

/// A gateway integration. Implementations own their own connection state
/// (HTTP client, credentials, target server) — none of that is threaded
/// through trait methods here, since it varies by backend (e.g.
/// apisix-standalone has no notion of a remote server to `ping`, only a
/// local cache to read/write).
///
/// `dyn Backend` is the CLI's dispatch mechanism for "which backend did the
/// user configure", so methods stay object-safe (boxed futures via
/// `#[async_trait]`, no generics).
///
/// Not carried over from the TS `Backend` interface: `on(eventType, cb)`
/// event subscription for task-progress/debug-request events. That was a
/// hand-rolled pub-sub built to feed the CLI's listr2 progress renderer and
/// axios debug logging. Its two jobs map directly onto `tracing`
/// instrumentation instead — `TASK_START`/`TASK_DONE` become a `tracing`
/// span's enter/exit, `AXIOS_DEBUG` becomes a `tracing::debug!` call at the
/// request site — so implementations emit spans/events directly rather than
/// through a bespoke bus on this trait.
#[async_trait]
pub trait Backend: Send + Sync {
    fn metadata(&self) -> BackendMetadata;

    async fn ping(&self) -> Result<(), BackendError>;

    async fn version(&self) -> Result<Version, BackendError>;

    async fn default_value(&self) -> Result<DefaultValue, BackendError>;

    async fn dump(&self) -> Result<Configuration, BackendError>;

    /// Applies `events`. Per-event failures are captured as individual
    /// `BackendSyncResult`s with `success: false` rather than failing the
    /// whole call — *unless* `opts.exit_on_failure` is set (the default):
    /// then the first failure aborts the whole call and is returned as
    /// `Err`, discarding any results accumulated so far, mirroring the TS
    /// implementation's `Observable` erroring out (via `throwError`) instead
    /// of completing with a partial list. Concurrency (per
    /// `opts.concurrent`) is an implementation detail of each backend, not
    /// something the trait signature encodes.
    async fn sync(&self, events: Vec<Event>, opts: BackendSyncOptions) -> Result<Vec<BackendSyncResult>, BackendError>;

    /// Not every backend can pre-validate events against the remote server
    /// before applying them; the default rejects with `Unsupported`,
    /// matching the TS interface's `validate?` being absent.
    async fn validate(&self, _events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        Err(BackendError::Unsupported("validate".into()))
    }

    async fn support_stream_route(&self) -> Result<bool, BackendError> {
        Ok(false)
    }
}
