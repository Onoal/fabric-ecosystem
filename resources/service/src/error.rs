#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ServiceError {
    #[error("service is unavailable")]
    Unavailable,
    #[error("service input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("service was not found")]
    NotFound,
    #[error("service integrity failure: {message}")]
    Integrity { message: String },
    #[error("service persistence failure: {message}")]
    Persistence { message: String },
    #[error("service has no live target")]
    NoTarget,
    #[error("service target publication failed: {message}")]
    PublishFailed { message: String },
    #[error("service dispatch failed: {message}")]
    DispatchFailed { message: String },
}

impl ServiceError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub(crate) fn integrity(message: impl Into<String>) -> Self {
        Self::Integrity {
            message: message.into(),
        }
    }

    pub(crate) fn persistence(message: impl Into<String>) -> Self {
        Self::Persistence {
            message: message.into(),
        }
    }
}
