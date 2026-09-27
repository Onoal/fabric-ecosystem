use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinuxHostErrorKind {
    UnsupportedPlatform,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxHostError {
    pub kind: LinuxHostErrorKind,
    pub detail: String,
}

impl LinuxHostError {
    pub fn new(kind: LinuxHostErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn unsupported_platform() -> Self {
        Self::new(
            LinuxHostErrorKind::UnsupportedPlatform,
            "linux host detection is only supported on target_os = linux",
        )
    }
}

impl fmt::Display for LinuxHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for LinuxHostError {}
