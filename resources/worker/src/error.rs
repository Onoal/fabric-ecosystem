#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkerError {
    #[error("worker is unavailable")]
    Unavailable,
    #[error("workload is incompatible: {issues:?}")]
    Incompatible {
        issues: Vec<crate::CompatibilityIssue>,
    },
    #[error("worker input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("worker protocol violation: {message}")]
    ProtocolViolation { message: String },
    #[error("worker prepare failed: {message}")]
    PrepareFailed { message: String },
    #[error("worker start failed: {message}")]
    StartFailed { message: String },
    #[error("worker http dispatch failed: {message}")]
    DispatchFailed { message: String },
    #[error("worker stop failed: {message}")]
    StopFailed { message: String },
}

impl WorkerError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
}
