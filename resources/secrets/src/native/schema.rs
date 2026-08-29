use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::SecretsError;

const SCHEMA_EPOCH: &str = "fabric.sec0.secrets.genesis";
const SCHEMA_VERSION: i64 = 1;

pub(crate) fn initialize_schema(
    connection: &mut Connection,
    path: &Path,
) -> Result<(), SecretsError> {
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(schema_error("enable foreign keys"))?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(schema_error("enable WAL"))?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(schema_error("set synchronous mode"))?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(schema_error("begin schema transaction"))?;
    if table_exists(&transaction, "secrets_schema")? {
        validate_genesis_schema(&transaction)?;
    } else {
        reject_pre_sec0_state(&transaction)?;
        install_genesis_schema(&transaction)?;
    }
    transaction
        .commit()
        .map_err(schema_error("commit schema transaction"))?;
    enforce_private_permissions(path)?;
    Ok(())
}

fn install_genesis_schema(connection: &Connection) -> Result<(), SecretsError> {
    connection
        .execute_batch(
            "
            CREATE TABLE secrets_schema (
                singleton_key INTEGER PRIMARY KEY CHECK (singleton_key = 1),
                schema_epoch TEXT NOT NULL CHECK (schema_epoch = 'fabric.sec0.secrets.genesis'),
                schema_version INTEGER NOT NULL CHECK (schema_version = 1)
            );
            CREATE TABLE secrets (
                secret_id TEXT PRIMARY KEY,
                scope_id TEXT NOT NULL,
                current_version INTEGER NOT NULL CHECK (current_version >= 1),
                deleted INTEGER NOT NULL CHECK (deleted IN (0, 1))
            );
            CREATE TABLE secret_versions (
                secret_id TEXT NOT NULL REFERENCES secrets(secret_id) ON DELETE CASCADE,
                version INTEGER NOT NULL CHECK (version >= 1),
                material TEXT NOT NULL,
                PRIMARY KEY (secret_id, version)
            );
            INSERT INTO secrets_schema (singleton_key, schema_epoch, schema_version)
                VALUES (1, 'fabric.sec0.secrets.genesis', 1);
            ",
        )
        .map_err(schema_error("install SEC0 secrets genesis schema"))
}

fn reject_pre_sec0_state(connection: &Connection) -> Result<(), SecretsError> {
    if table_exists(connection, "secrets")? || table_exists(connection, "secret_versions")? {
        return Err(SecretsError::integrity(
            "unsupported pre-SEC0 secrets state without genesis marker",
        ));
    }
    Ok(())
}

fn validate_genesis_schema(connection: &Connection) -> Result<(), SecretsError> {
    require_columns(
        connection,
        "secrets_schema",
        &["singleton_key", "schema_epoch", "schema_version"],
    )?;
    require_columns(
        connection,
        "secrets",
        &["secret_id", "scope_id", "current_version", "deleted"],
    )?;
    require_columns(
        connection,
        "secret_versions",
        &["secret_id", "version", "material"],
    )?;
    let marker = connection
        .query_row(
            "SELECT schema_epoch, schema_version FROM secrets_schema WHERE singleton_key = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(integrity_error("unsupported SEC0 secrets schema metadata"))?;
    match marker {
        Some((epoch, SCHEMA_VERSION)) if epoch == SCHEMA_EPOCH => Ok(()),
        _ => Err(SecretsError::integrity(
            "unsupported SEC0 secrets schema marker",
        )),
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, SecretsError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(schema_error("inspect schema tables"))
}

fn require_columns(
    connection: &Connection,
    table: &str,
    expected: &[&str],
) -> Result<(), SecretsError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(integrity_error("unsupported SEC0 secrets schema shape"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(integrity_error("unsupported SEC0 secrets schema shape"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(integrity_error("unsupported SEC0 secrets schema shape"))?;
    if expected
        .iter()
        .all(|column| columns.iter().any(|actual| actual == column))
    {
        Ok(())
    } else {
        Err(SecretsError::integrity(
            "unsupported SEC0 secrets schema shape",
        ))
    }
}

#[cfg(unix)]
fn enforce_private_permissions(path: &Path) -> Result<(), SecretsError> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|error| {
        SecretsError::persistence(format!("failed to stat secret store: {error}"))
    })?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions).map_err(|error| {
        SecretsError::persistence(format!(
            "failed to tighten secret store permissions: {error}"
        ))
    })
}

#[cfg(not(unix))]
fn enforce_private_permissions(_path: &Path) -> Result<(), SecretsError> {
    Ok(())
}

fn schema_error(action: &'static str) -> impl FnOnce(rusqlite::Error) -> SecretsError {
    move |error| SecretsError::persistence(format!("failed to {action}: {error}"))
}

fn integrity_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> SecretsError {
    move |error| SecretsError::integrity(format!("{context}: {error}"))
}
