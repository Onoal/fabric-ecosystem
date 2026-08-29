use std::collections::BTreeMap;

use crate::AuthorityError;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthorityScopeId(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorRef(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityDecision {
    Allow,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRef {
    pub scope_id: AuthorityScopeId,
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestContext {
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityRequest {
    pub actor: ActorRef,
    pub action: ActionId,
    pub resource: ResourceRef,
    pub context: RequestContext,
}

impl AuthorityScopeId {
    pub fn parse(value: impl Into<String>) -> Result<Self, AuthorityError> {
        parse_identifier(value.into(), "authority scope id").map(Self)
    }

    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ActorRef {
    pub fn new(value: impl Into<String>) -> Result<Self, AuthorityError> {
        let value = parse_identifier(value.into(), "actor ref")?;
        validate_actor_ref(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ActionId {
    pub fn new(value: impl Into<String>) -> Result<Self, AuthorityError> {
        parse_identifier(value.into(), "action id").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ResourceRef {
    pub fn new(
        scope_id: AuthorityScopeId,
        kind: impl Into<String>,
        id: impl Into<String>,
    ) -> Result<Self, AuthorityError> {
        let kind = parse_identifier(kind.into(), "resource kind")?;
        let id = parse_identifier(id.into(), "resource id")?;
        Ok(Self { scope_id, kind, id })
    }
}

pub(crate) fn parse_identifier(value: String, label: &str) -> Result<String, AuthorityError> {
    if value.is_empty()
        || value.len() > 128
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(AuthorityError::invalid_input(format!("invalid {label}")));
    }
    Ok(value)
}

fn validate_actor_ref(value: &str) -> Result<(), AuthorityError> {
    let Some((namespace, local)) = value.split_once(':') else {
        return Err(AuthorityError::invalid_input(
            "actor ref must contain namespace separator",
        ));
    };
    if namespace.is_empty() || local.is_empty() {
        return Err(AuthorityError::invalid_input(
            "actor ref must include non-empty namespace and local id",
        ));
    }
    if !namespace
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(AuthorityError::invalid_input(
            "actor ref namespace contains invalid characters",
        ));
    }
    if local.bytes().any(|byte| byte == b':') {
        return Err(AuthorityError::invalid_input(
            "actor ref local id may not contain additional namespace separators",
        ));
    }
    if local
        .bytes()
        .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
    {
        return Err(AuthorityError::invalid_input(
            "actor ref local id contains invalid characters",
        ));
    }
    Ok(())
}
