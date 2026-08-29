use std::path::Path;

use fabric_resource_authority::{
    ActorRef, AuthorityContract, AuthorityDecision, AuthorityRequest, AuthorityScopeId,
    RequestContext,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::error::SecretsError;
use crate::model::{
    MaterializedSecret, SecretId, SecretMaterial, SecretRef, SecretVersion, StoredSecret,
    secret_collection_resource, secret_create_action, secret_delete_action,
    secret_materialize_action, secret_rotate_action,
};
use crate::native::schema::initialize_schema;

#[derive(Debug)]
pub(crate) struct SecretsDatabase {
    connection: Connection,
    #[cfg(test)]
    failpoint: Option<PersistenceFailpoint>,
}

impl SecretsDatabase {
    pub(crate) fn open(path: &Path) -> Result<Self, SecretsError> {
        let mut connection = Connection::open(path).map_err(|error| {
            SecretsError::persistence(format!("failed to open secrets database: {error}"))
        })?;
        initialize_schema(&mut connection, path)?;
        Ok(Self {
            connection,
            #[cfg(test)]
            failpoint: None,
        })
    }

    pub(crate) fn create(
        &mut self,
        authority: &AuthorityContract,
        actor: &ActorRef,
        scope_id: &AuthorityScopeId,
        value: SecretMaterial,
    ) -> Result<StoredSecret, SecretsError> {
        let resource = secret_collection_resource(scope_id)?;
        authorize(authority, actor, &secret_create_action(), resource)?;
        let secret = SecretRef::new(SecretId::new(uuid_fragment()), scope_id.clone());
        let version = SecretVersion::initial();
        let fail_after_metadata =
            self.take_failpoint(PersistenceFailpoint::CreateAfterMetadataInsert);
        let transaction = self.connection.transaction().map_err(|error| {
            SecretsError::persistence(format!("failed to begin create transaction: {error}"))
        })?;
        transaction
            .execute(
                "INSERT INTO secrets (secret_id, scope_id, current_version, deleted) VALUES (?1, ?2, ?3, 0)",
                params![secret.id.as_str(), secret.scope_id.as_str(), version.as_i64()],
            )
            .map_err(|error| {
                SecretsError::persistence(format!("failed to insert secret metadata: {error}"))
            })?;
        if fail_after_metadata {
            return Err(SecretsError::persistence(
                "injected secrets persistence failure at create_after_metadata_insert",
            ));
        }
        insert_version(&transaction, &secret, &version, &value)?;
        transaction.commit().map_err(|error| {
            SecretsError::persistence(format!("failed to commit create transaction: {error}"))
        })?;
        Ok(StoredSecret { secret, version })
    }

    pub(crate) fn materialize(
        &self,
        authority: &AuthorityContract,
        actor: &ActorRef,
        secret: &SecretRef,
    ) -> Result<MaterializedSecret, SecretsError> {
        authorize(
            authority,
            actor,
            &secret_materialize_action(),
            secret.authority_resource()?,
        )?;
        let record = self.load_secret(secret)?;
        if record.deleted {
            return Err(SecretsError::NotFound);
        }
        let value = self.load_material(secret, &record.current_version)?;
        Ok(MaterializedSecret {
            secret: secret.clone(),
            version: record.current_version,
            value,
        })
    }

    pub(crate) fn rotate(
        &mut self,
        authority: &AuthorityContract,
        actor: &ActorRef,
        secret: &SecretRef,
        value: SecretMaterial,
    ) -> Result<SecretVersion, SecretsError> {
        authorize(
            authority,
            actor,
            &secret_rotate_action(),
            secret.authority_resource()?,
        )?;
        let record = self.load_secret(secret)?;
        if record.deleted {
            return Err(SecretsError::NotFound);
        }
        let next_version = record.current_version.next();
        let fail_after_version_insert =
            self.take_failpoint(PersistenceFailpoint::RotateAfterVersionInsert);
        let transaction = self.connection.transaction().map_err(|error| {
            SecretsError::persistence(format!("failed to begin rotate transaction: {error}"))
        })?;
        insert_version(&transaction, secret, &next_version, &value)?;
        if fail_after_version_insert {
            return Err(SecretsError::persistence(
                "injected secrets persistence failure at rotate_after_version_insert",
            ));
        }
        transaction
            .execute(
                "UPDATE secrets SET current_version = ?2 WHERE secret_id = ?1",
                params![secret.id.as_str(), next_version.as_i64()],
            )
            .map_err(|error| {
                SecretsError::persistence(format!("failed to update secret version: {error}"))
            })?;
        transaction.commit().map_err(|error| {
            SecretsError::persistence(format!("failed to commit rotate transaction: {error}"))
        })?;
        Ok(next_version)
    }

    pub(crate) fn delete(
        &mut self,
        authority: &AuthorityContract,
        actor: &ActorRef,
        secret: &SecretRef,
    ) -> Result<(), SecretsError> {
        authorize(
            authority,
            actor,
            &secret_delete_action(),
            secret.authority_resource()?,
        )?;
        let record = self.load_secret(secret)?;
        if record.deleted {
            return Err(SecretsError::NotFound);
        }
        self.connection
            .execute(
                "UPDATE secrets SET deleted = 1 WHERE secret_id = ?1",
                params![secret.id.as_str()],
            )
            .map_err(|error| {
                SecretsError::persistence(format!("failed to delete secret: {error}"))
            })?;
        Ok(())
    }

    fn load_secret(&self, secret: &SecretRef) -> Result<PersistedSecret, SecretsError> {
        self.connection
            .query_row(
                "SELECT scope_id, current_version, deleted FROM secrets WHERE secret_id = ?1",
                params![secret.id.as_str()],
                |row| {
                    Ok(PersistedSecret {
                        scope_id: AuthorityScopeId::parse(row.get::<_, String>(0)?)
                            .map_err(|error| to_sql_error(map_authority_error(error)))?,
                        current_version: SecretVersion::parse(row.get::<_, i64>(1)?)
                            .map_err(to_sql_error)?,
                        deleted: row.get::<_, i64>(2)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|error| {
                SecretsError::persistence(format!("failed to load secret metadata: {error}"))
            })?
            .ok_or(SecretsError::NotFound)
            .and_then(|record| {
                if record.scope_id == secret.scope_id {
                    Ok(record)
                } else {
                    Err(SecretsError::Integrity {
                        message: "secret scope does not match requested reference".to_owned(),
                    })
                }
            })
    }

    fn load_material(
        &self,
        secret: &SecretRef,
        version: &SecretVersion,
    ) -> Result<SecretMaterial, SecretsError> {
        self.connection
            .query_row(
                "SELECT material FROM secret_versions WHERE secret_id = ?1 AND version = ?2",
                params![secret.id.as_str(), version.as_i64()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                SecretsError::persistence(format!("failed to load secret material: {error}"))
            })?
            .map(SecretMaterial::new)
            .ok_or_else(|| {
                SecretsError::integrity("secret material for current version is missing")
            })
    }

    #[cfg(test)]
    pub(crate) fn inject_failpoint(&mut self, failpoint: PersistenceFailpoint) {
        self.failpoint = Some(failpoint);
    }

    fn take_failpoint(&mut self, failpoint: PersistenceFailpoint) -> bool {
        #[cfg(test)]
        if self.failpoint == Some(failpoint) {
            self.failpoint = None;
            return true;
        }

        let _ = failpoint;
        false
    }
}

#[derive(Debug)]
struct PersistedSecret {
    scope_id: AuthorityScopeId,
    current_version: SecretVersion,
    deleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PersistenceFailpoint {
    CreateAfterMetadataInsert,
    RotateAfterVersionInsert,
}

impl std::fmt::Display for PersistenceFailpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateAfterMetadataInsert => f.write_str("create_after_metadata_insert"),
            Self::RotateAfterVersionInsert => f.write_str("rotate_after_version_insert"),
        }
    }
}

fn insert_version(
    connection: &Transaction<'_>,
    secret: &SecretRef,
    version: &SecretVersion,
    value: &SecretMaterial,
) -> Result<(), SecretsError> {
    connection
        .execute(
            "INSERT INTO secret_versions (secret_id, version, material) VALUES (?1, ?2, ?3)",
            params![secret.id.as_str(), version.as_i64(), value.expose()],
        )
        .map_err(|error| {
            SecretsError::persistence(format!("failed to insert secret version: {error}"))
        })?;
    Ok(())
}

fn authorize(
    authority: &AuthorityContract,
    actor: &ActorRef,
    action: &fabric_resource_authority::ActionId,
    resource: fabric_resource_authority::ResourceRef,
) -> Result<(), SecretsError> {
    let decision = authority
        .authorize(&AuthorityRequest {
            actor: actor.clone(),
            action: action.clone(),
            resource,
            context: RequestContext::default(),
        })
        .map_err(map_authority_error)?;
    match decision {
        AuthorityDecision::Allow => Ok(()),
        AuthorityDecision::Deny => Err(SecretsError::AccessDenied),
    }
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

fn to_sql_error(error: SecretsError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(error.to_string())),
    )
}

fn uuid_fragment() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}
