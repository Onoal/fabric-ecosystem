use std::fmt;

use fabric_component::Component;

use crate::NamespaceError;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NamespaceName(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamespaceClaim {
    owner: Component,
    name: NamespaceName,
}

impl NamespaceName {
    pub fn new(value: impl Into<String>) -> Result<Self, NamespaceError> {
        let value = value.into();
        validate_namespace_name(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl NamespaceClaim {
    pub fn new(owner: Component, name: NamespaceName) -> Self {
        Self { owner, name }
    }

    pub fn owner(&self) -> &Component {
        &self.owner
    }

    pub fn name(&self) -> &NamespaceName {
        &self.name
    }
}

impl fmt::Display for NamespaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_namespace_name(value: &str) -> Result<(), NamespaceError> {
    if value.trim() != value || value.is_empty() {
        return Err(NamespaceError::InvalidNamespaceName(value.to_owned()));
    }
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Ok(());
    }
    Err(NamespaceError::InvalidNamespaceName(value.to_owned()))
}
