#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Backend(#[from] adc_sdk::BackendError),
    #[error(transparent)]
    Convert(#[from] adc_sdk::ConvertError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("{0}")]
    GlobPattern(#[from] glob::PatternError),

    /// A failure whose message was already printed by a per-event line
    /// (`sync_slots`/`sync_report`/`sync_debug`) as it happened — `main`
    /// only needs the exit code, not a second copy of the same text.
    #[error("")]
    AlreadyReported,
}

impl CliError {
    pub fn msg(message: impl Into<String>) -> Self {
        CliError::Message(message.into())
    }
}
