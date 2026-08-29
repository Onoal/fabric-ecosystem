use rusqlite::{Connection, OptionalExtension};

use crate::ConnectivityError;

const CONNECTIVITY_SCHEMA_EPOCH: &str = "fabric.conn0.connectivity.genesis";
const CONNECTIVITY_SCHEMA_VERSION: i64 = 1;

pub(crate) fn initialize_schema(connection: &mut Connection) -> Result<(), ConnectivityError> {
    reject_legacy_schema(connection)?;
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = FULL;

            CREATE TABLE IF NOT EXISTS connectivity_meta (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                schema_epoch TEXT NOT NULL,
                schema_version INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS reachabilities (
                reachability_id TEXT PRIMARY KEY,
                ingress_route_id TEXT NOT NULL,
                connectivity_scope TEXT NOT NULL,
                UNIQUE(ingress_route_id, connectivity_scope)
            );
            ",
        )
        .map_err(|error| {
            ConnectivityError::persistence(format!("initialize connectivity schema: {error}"))
        })?;
    connection
        .execute(
            "
            INSERT INTO connectivity_meta (
                singleton,
                schema_epoch,
                schema_version
            ) VALUES (1, ?1, ?2)
            ON CONFLICT(singleton) DO NOTHING
            ",
            rusqlite::params![CONNECTIVITY_SCHEMA_EPOCH, CONNECTIVITY_SCHEMA_VERSION],
        )
        .map_err(|error| {
            ConnectivityError::persistence(format!("insert connectivity schema marker: {error}"))
        })?;
    validate_schema_marker(connection)?;
    Ok(())
}

fn reject_legacy_schema(connection: &Connection) -> Result<(), ConnectivityError> {
    let has_legacy_routes = table_exists(connection, "routes")?;
    let has_meta = table_exists(connection, "connectivity_meta")?;
    if has_legacy_routes && !has_meta {
        return Err(ConnectivityError::integrity(
            "unsupported legacy connectivity schema; recreate connectivity state for CONN0",
        ));
    }
    Ok(())
}

fn validate_schema_marker(connection: &Connection) -> Result<(), ConnectivityError> {
    let (epoch, version): (String, i64) = connection
        .query_row(
            "
            SELECT schema_epoch, schema_version
            FROM connectivity_meta
            WHERE singleton = 1
            ",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| {
            ConnectivityError::persistence(format!("read connectivity schema marker: {error}"))
        })?;
    if epoch == CONNECTIVITY_SCHEMA_EPOCH && version == CONNECTIVITY_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConnectivityError::integrity(format!(
            "unsupported connectivity schema marker {epoch}:{version}"
        )))
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, ConnectivityError> {
    connection
        .query_row(
            "
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table' AND name = ?1
            ",
            rusqlite::params![table],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(|error| {
            ConnectivityError::persistence(format!("check connectivity table existence: {error}"))
        })
}
