use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub enum PreparedWorkloadEnvironmentValue {
    Public(String),
    Sensitive(SensitiveProjectionValue),
}

#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveProjectionValue(String);

impl PreparedWorkloadEnvironmentValue {
    pub fn public(value: impl Into<String>) -> Self {
        Self::Public(value.into())
    }

    pub fn sensitive(value: impl Into<String>) -> Self {
        Self::Sensitive(SensitiveProjectionValue::new(value))
    }

    pub fn expose(&self) -> &str {
        match self {
            Self::Public(value) => value,
            Self::Sensitive(value) => value.expose(),
        }
    }

    pub fn is_sensitive(&self) -> bool {
        matches!(self, Self::Sensitive(_))
    }
}

impl SensitiveProjectionValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PreparedWorkloadEnvironmentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public(value) => f.debug_struct("Public").field("len", &value.len()).finish(),
            Self::Sensitive(_) => f.debug_tuple("Sensitive").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Debug for SensitiveProjectionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SensitiveProjectionValue(<redacted>)")
    }
}
