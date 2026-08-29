#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("authority is unavailable")]
    Unavailable,

    #[error("authority input is invalid: {message}")]
    InvalidInput { message: String },

    #[error("authority persistence failed: {message}")]
    Persistence { message: String },

    #[error("authority integrity check failed: {message}")]
    Integrity { message: String },
}

impl AuthorityError {
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
