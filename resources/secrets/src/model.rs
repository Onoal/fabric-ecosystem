use std::fmt;

use fabric_resource_authority::{ActionId, ActorRef, AuthorityScopeId, ResourceRef};
use secrecy::{ExposeSecret, SecretString};

use crate::SecretsError;

const SECRET_COLLECTION_RESOURCE_ID: &str = "scope";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretId(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretVersion(i64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretRef {
    pub id: SecretId,
    pub scope_id: AuthorityScopeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedSecretRef {
    secret: SecretRef,
    actor: ActorRef,
}

#[derive(Clone)]
pub struct SecretMaterial(SecretString);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSecret {
    pub secret: SecretRef,
    pub version: SecretVersion,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MaterializedSecret {
    pub secret: SecretRef,
    pub version: SecretVersion,
    pub value: SecretMaterial,
}

impl SecretId {
    pub fn parse(value: impl Into<String>) -> Result<Self, SecretsError> {
        parse_identifier(value.into(), "secret id").map(Self)
    }

    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SecretVersion {
    pub(crate) fn initial() -> Self {
        Self(1)
    }

    pub(crate) fn parse(value: i64) -> Result<Self, SecretsError> {
        if value < 1 {
            return Err(SecretsError::integrity("secret version must be positive"));
        }
        Ok(Self(value))
    }

    pub(crate) fn next(&self) -> Self {
        Self(self.0 + 1)
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl SecretRef {
    pub fn new(id: SecretId, scope_id: AuthorityScopeId) -> Self {
        Self { id, scope_id }
    }

    pub fn authority_resource(&self) -> Result<ResourceRef, SecretsError> {
        ResourceRef::new(self.scope_id.clone(), "secret", self.id.as_str())
            .map_err(map_authority_error)
    }
}

impl AuthorizedSecretRef {
    pub fn new(secret: SecretRef, actor: ActorRef) -> Self {
        Self { secret, actor }
    }

    pub fn secret(&self) -> &SecretRef {
        &self.secret
    }

    pub fn actor(&self) -> &ActorRef {
        &self.actor
    }
}

impl SecretMaterial {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::new(value.into().into_boxed_str()))
    }

    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl PartialEq for SecretMaterial {
    fn eq(&self, other: &Self) -> bool {
        self.expose() == other.expose()
    }
}

impl Eq for SecretMaterial {}

impl fmt::Debug for SecretMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretMaterial([REDACTED])")
    }
}

impl fmt::Display for SecretMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Debug for MaterializedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MaterializedSecret")
            .field("secret", &self.secret)
            .field("version", &self.version)
            .field("value", &"SecretMaterial([REDACTED])")
            .finish()
    }
}

pub fn secret_create_action() -> ActionId {
    ActionId::new("fabric.secret.create").expect("static secret create action")
}

pub fn secret_materialize_action() -> ActionId {
    ActionId::new("fabric.secret.materialize").expect("static secret materialize action")
}

pub fn secret_rotate_action() -> ActionId {
    ActionId::new("fabric.secret.rotate").expect("static secret rotate action")
}

pub fn secret_delete_action() -> ActionId {
    ActionId::new("fabric.secret.delete").expect("static secret delete action")
}

pub(crate) fn secret_collection_resource(
    scope_id: &AuthorityScopeId,
) -> Result<ResourceRef, SecretsError> {
    ResourceRef::new(
        scope_id.clone(),
        "secret-collection",
        SECRET_COLLECTION_RESOURCE_ID,
    )
    .map_err(map_authority_error)
}

fn parse_identifier(value: String, label: &str) -> Result<String, SecretsError> {
    if value.is_empty()
        || value.len() > 128
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(SecretsError::invalid_input(format!("invalid {label}")));
    }
    Ok(value)
}

fn map_authority_error(error: fabric_resource_authority::AuthorityError) -> SecretsError {
    match error {
        fabric_resource_authority::AuthorityError::Unavailable => SecretsError::Unavailable,
        fabric_resource_authority::AuthorityError::InvalidInput { message } => {
            SecretsError::InvalidInput { message }
        }
        fabric_resource_authority::AuthorityError::Persistence { message } => {
            SecretsError::Persistence {
                message: format!("authority operation failed: {message}"),
            }
        }
        fabric_resource_authority::AuthorityError::Integrity { message } => {
            SecretsError::Integrity {
                message: format!("authority integrity failed: {message}"),
            }
        }
    }
}
