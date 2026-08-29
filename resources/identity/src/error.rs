#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error("identity is unavailable")]
    Unavailable,

    #[error("identity input is invalid: {message}")]
    InvalidInput { message: String },

    #[error("identity persistence failed: {message}")]
    Persistence { message: String },

    #[error("identity integrity check failed: {message}")]
    Integrity { message: String },
}

impl IdentityError {
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
