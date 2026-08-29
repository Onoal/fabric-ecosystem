use rusqlite::{Connection, OptionalExtension};

use crate::AuthorityError;

const SCHEMA_EPOCH: &str = "fabric.sec0.authority.genesis";
const SCHEMA_VERSION: i64 = 1;

pub(crate) fn initialize_schema(connection: &mut Connection) -> Result<(), AuthorityError> {
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
    if table_exists(&transaction, "authority_schema")? {
        validate_genesis_schema(&transaction)?;
    } else {
        reject_pre_sec0_state(&transaction)?;
        install_genesis_schema(&transaction)?;
    }
    transaction
        .commit()
        .map_err(schema_error("commit schema transaction"))
}

fn install_genesis_schema(connection: &Connection) -> Result<(), AuthorityError> {
    connection
        .execute_batch(
            "
            CREATE TABLE authority_schema (
                singleton_key INTEGER PRIMARY KEY CHECK (singleton_key = 1),
                schema_epoch TEXT NOT NULL CHECK (schema_epoch = 'fabric.sec0.authority.genesis'),
                schema_version INTEGER NOT NULL CHECK (schema_version = 1)
            );
            CREATE TABLE scopes (scope_id TEXT PRIMARY KEY);
            CREATE TABLE grants (
                grant_kind TEXT NOT NULL CHECK (grant_kind IN ('scope_control', 'exact')),
                actor_ref TEXT NOT NULL,
                scope_id TEXT NOT NULL REFERENCES scopes(scope_id) ON DELETE CASCADE,
                action_id TEXT,
                resource_kind TEXT,
                resource_id TEXT
            );
            CREATE UNIQUE INDEX grants_unique ON grants (
                grant_kind, actor_ref, scope_id,
                IFNULL(action_id, ''), IFNULL(resource_kind, ''), IFNULL(resource_id, '')
            );
            INSERT INTO authority_schema (singleton_key, schema_epoch, schema_version)
                VALUES (1, 'fabric.sec0.authority.genesis', 1);
            ",
        )
        .map_err(schema_error("install SEC0 genesis schema"))
}

fn reject_pre_sec0_state(connection: &Connection) -> Result<(), AuthorityError> {
    if table_exists(connection, "scopes")? || table_exists(connection, "grants")? {
        return Err(AuthorityError::integrity(
            "unsupported pre-SEC0 authority state without genesis marker",
        ));
    }
    Ok(())
}

fn validate_genesis_schema(connection: &Connection) -> Result<(), AuthorityError> {
    require_columns(
        connection,
        "authority_schema",
        &["singleton_key", "schema_epoch", "schema_version"],
    )?;
    require_columns(connection, "scopes", &["scope_id"])?;
    require_columns(
        connection,
        "grants",
        &[
            "grant_kind",
            "actor_ref",
            "scope_id",
            "action_id",
            "resource_kind",
            "resource_id",
        ],
    )?;
    if !index_exists(connection, "grants_unique")? {
        return Err(AuthorityError::integrity(
            "unsupported pre-SEC0 authority schema is missing grants uniqueness",
        ));
    }
    let marker = connection
        .query_row(
            "SELECT schema_epoch, schema_version FROM authority_schema WHERE singleton_key = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(integrity_error(
            "unsupported pre-SEC0 authority schema metadata",
        ))?;
    match marker {
        Some((epoch, SCHEMA_VERSION)) if epoch == SCHEMA_EPOCH => Ok(()),
        _ => Err(AuthorityError::integrity(
            "unsupported pre-SEC0 authority schema marker",
        )),
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, AuthorityError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(schema_error("inspect schema tables"))
}

fn index_exists(connection: &Connection, index: &str) -> Result<bool, AuthorityError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1)",
            [index],
            |row| row.get(0),
        )
        .map_err(schema_error("inspect schema indexes"))
}

fn require_columns(
    connection: &Connection,
    table: &str,
    expected: &[&str],
) -> Result<(), AuthorityError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(integrity_error(
            "unsupported pre-SEC0 authority schema shape",
        ))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(integrity_error(
            "unsupported pre-SEC0 authority schema shape",
        ))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(integrity_error(
            "unsupported pre-SEC0 authority schema shape",
        ))?;
    if expected
        .iter()
        .all(|column| columns.iter().any(|actual| actual == column))
    {
        Ok(())
    } else {
        Err(AuthorityError::integrity(
            "unsupported pre-SEC0 authority schema shape",
        ))
    }
}

fn schema_error(action: &'static str) -> impl FnOnce(rusqlite::Error) -> AuthorityError {
    move |error| AuthorityError::persistence(format!("failed to {action}: {error}"))
}

fn integrity_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> AuthorityError {
    move |error| AuthorityError::integrity(format!("{context}: {error}"))
}
