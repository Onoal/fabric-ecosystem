#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SecretsError {
    #[error("secrets are unavailable")]
    Unavailable,

    #[error("secret input is invalid: {message}")]
    InvalidInput { message: String },

    #[error("secret access is denied")]
    AccessDenied,

    #[error("secret was not found")]
    NotFound,

    #[error("secret persistence failed: {message}")]
    Persistence { message: String },

    #[error("secret integrity check failed: {message}")]
    Integrity { message: String },
}

impl SecretsError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub(crate) fn persistence(message: impl Into<String>) -> Self {
        Self::Persistence {
            message: message.into(),
        }
    }

    pub(crate) fn integrity(message: impl Into<String>) -> Self {
        Self::Integrity {
            message: message.into(),
        }
    }
}
