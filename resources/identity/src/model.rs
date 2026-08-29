#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    pub id: PrincipalId,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrincipalId(String);

impl PrincipalId {
    pub fn parse(value: impl Into<String>) -> Result<Self, crate::IdentityError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || value
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        {
            return Err(crate::IdentityError::invalid_input("invalid principal id"));
        }
        Ok(Self(value))
    }

    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
