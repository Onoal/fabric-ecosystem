use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyValueErrorKind {
    ReadFailed,
    WriteFailed,
    DeleteFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyValueError {
    pub kind: KeyValueErrorKind,
    pub detail: String,
}

impl KeyValueError {
    pub fn new(kind: KeyValueErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn read_failed(detail: impl Into<String>) -> Self {
        Self::new(KeyValueErrorKind::ReadFailed, detail)
    }

    pub fn write_failed(detail: impl Into<String>) -> Self {
        Self::new(KeyValueErrorKind::WriteFailed, detail)
    }

    pub fn delete_failed(detail: impl Into<String>) -> Self {
        Self::new(KeyValueErrorKind::DeleteFailed, detail)
    }
}

impl fmt::Display for KeyValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for KeyValueError {}
