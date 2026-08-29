use rusqlite::{Connection, OptionalExtension};

use crate::ServiceError;

pub(crate) const CURRENT_SCHEMA_VERSION: i64 = 1;

pub(crate) fn initialize_schema(connection: &mut Connection) -> Result<(), ServiceError> {
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| ServiceError::persistence(format!("enable foreign keys: {error}")))?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| ServiceError::persistence(format!("enable WAL: {error}")))?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|error| ServiceError::persistence(format!("set synchronous mode: {error}")))?;

    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| ServiceError::persistence(format!("begin schema transaction: {error}")))?;
    transaction
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS services_schema (
                singleton_key INTEGER PRIMARY KEY CHECK (singleton_key = 1),
                schema_version INTEGER NOT NULL
            );
            ",
        )
        .map_err(|error| ServiceError::persistence(format!("create schema table: {error}")))?;

    let version = transaction
        .query_row(
            "SELECT schema_version FROM services_schema WHERE singleton_key = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| ServiceError::persistence(format!("read schema version: {error}")))?;
    match version {
        None => install_schema_v1(&transaction)?,
        Some(version) if version == CURRENT_SCHEMA_VERSION => {}
        Some(version) if version > CURRENT_SCHEMA_VERSION => {
            return Err(ServiceError::integrity(format!(
                "schema version {version} is newer than supported version {CURRENT_SCHEMA_VERSION}"
            )));
        }
        Some(version) => {
            return Err(ServiceError::integrity(format!(
                "unsupported persisted schema version {version}"
            )));
        }
    }

    transaction
        .commit()
        .map_err(|error| ServiceError::persistence(format!("commit schema transaction: {error}")))
}

fn install_schema_v1(connection: &Connection) -> Result<(), ServiceError> {
    connection
        .execute_batch(
            "
            CREATE TABLE services (
                service_id TEXT PRIMARY KEY,
                service_scope TEXT NOT NULL,
                service_name TEXT NOT NULL,
                service_protocol TEXT NOT NULL,
                UNIQUE (service_scope, service_name)
            );

            INSERT INTO services_schema (singleton_key, schema_version) VALUES (1, 1);
            ",
        )
        .map_err(|error| ServiceError::persistence(format!("install schema v1: {error}")))
}
