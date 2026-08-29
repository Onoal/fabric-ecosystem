use rusqlite::{Connection, OptionalExtension};

use crate::IdentityError;

const SCHEMA_EPOCH: &str = "fabric.ia0.genesis";
const SCHEMA_VERSION: i64 = 1;

pub(crate) fn initialize_schema(connection: &mut Connection) -> Result<(), IdentityError> {
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| {
            IdentityError::persistence(format!("failed to enable foreign keys: {error}"))
        })?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| IdentityError::persistence(format!("failed to enable WAL: {error}")))?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|error| {
            IdentityError::persistence(format!("failed to set synchronous mode: {error}"))
        })?;

    let transaction = connection.unchecked_transaction().map_err(|error| {
        IdentityError::persistence(format!("failed to begin schema transaction: {error}"))
    })?;
    if table_exists(&transaction, "identity_schema")? {
        validate_genesis_schema(&transaction)?;
    } else {
        reject_pre_ia0_state(&transaction)?;
        install_genesis_schema(&transaction)?;
    }
    transaction.commit().map_err(|error| {
        IdentityError::persistence(format!("failed to commit schema transaction: {error}"))
    })
}

fn install_genesis_schema(connection: &Connection) -> Result<(), IdentityError> {
    connection
        .execute_batch(
            "
            CREATE TABLE identity_schema (
                singleton_key INTEGER PRIMARY KEY CHECK (singleton_key = 1),
                schema_epoch TEXT NOT NULL CHECK (schema_epoch = 'fabric.ia0.genesis'),
                schema_version INTEGER NOT NULL CHECK (schema_version = 1)
            );
            CREATE TABLE principals (principal_id TEXT PRIMARY KEY);
            INSERT INTO identity_schema (singleton_key, schema_epoch, schema_version)
                VALUES (1, 'fabric.ia0.genesis', 1);
            ",
        )
        .map_err(|error| {
            IdentityError::persistence(format!("failed to install IA0 genesis schema: {error}"))
        })
}

fn reject_pre_ia0_state(connection: &Connection) -> Result<(), IdentityError> {
    if table_exists(connection, "principals")? {
        return Err(IdentityError::integrity(
            "unsupported pre-IA0 identity state without genesis marker",
        ));
    }
    Ok(())
}

fn validate_genesis_schema(connection: &Connection) -> Result<(), IdentityError> {
    require_columns(
        connection,
        "identity_schema",
        &["singleton_key", "schema_epoch", "schema_version"],
    )?;
    require_columns(connection, "principals", &["principal_id"])?;
    let marker = connection
        .query_row(
            "SELECT schema_epoch, schema_version FROM identity_schema WHERE singleton_key = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|error| {
            IdentityError::integrity(format!(
                "unsupported pre-IA0 identity schema metadata: {error}"
            ))
        })?;
    match marker {
        Some((epoch, SCHEMA_VERSION)) if epoch == SCHEMA_EPOCH => Ok(()),
        _ => Err(IdentityError::integrity(
            "unsupported pre-IA0 identity schema marker",
        )),
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, IdentityError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|error| {
            IdentityError::persistence(format!("failed to inspect schema tables: {error}"))
        })
}

fn require_columns(
    connection: &Connection,
    table: &str,
    expected: &[&str],
) -> Result<(), IdentityError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| {
            IdentityError::integrity(format!(
                "unsupported pre-IA0 identity schema shape: {error}"
            ))
        })?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| {
            IdentityError::integrity(format!(
                "unsupported pre-IA0 identity schema shape: {error}"
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            IdentityError::integrity(format!(
                "unsupported pre-IA0 identity schema shape: {error}"
            ))
        })?;
    if expected
        .iter()
        .all(|column| columns.iter().any(|actual| actual == column))
    {
        Ok(())
    } else {
        Err(IdentityError::integrity(
            "unsupported pre-IA0 identity schema shape",
        ))
    }
}
