use std::path::Path;

use fabric_resource_ingress::IngressRouteId;
use rusqlite::{Connection, OptionalExtension, params};

use crate::native::schema::initialize_schema;
use crate::{
    ConnectivityError, ConnectivityScope, PreparedReachability, Reachability, ReachabilityId,
};

#[derive(Debug)]
pub(crate) struct ConnectivityDatabase {
    connection: Connection,
}

impl ConnectivityDatabase {
    pub(crate) fn open(path: &Path) -> Result<Self, ConnectivityError> {
        let mut connection = Connection::open(path).map_err(|error| {
            ConnectivityError::persistence(format!("open connectivity database: {error}"))
        })?;
        initialize_schema(&mut connection)?;
        let database = Self { connection };
        database.check_integrity()?;
        Ok(database)
    }

    pub(crate) fn ensure_reachability(
        &mut self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<PreparedReachability, ConnectivityError> {
        if let Some(reachability) = self.find_reachability(ingress_route_id, scope)? {
            return Ok(PreparedReachability {
                reachability,
                created: false,
            });
        }
        let reachability = Reachability::new(ingress_route_id.clone(), scope);
        self.connection
            .execute(
                "
                INSERT INTO reachabilities (
                    reachability_id,
                    ingress_route_id,
                    connectivity_scope
                ) VALUES (?1, ?2, ?3)
                ",
                params![
                    reachability.id.as_str(),
                    reachability.ingress_route_id.as_str(),
                    reachability.scope.as_str(),
                ],
            )
            .map_err(|error| {
                ConnectivityError::persistence(format!("insert reachability: {error}"))
            })?;
        Ok(PreparedReachability {
            reachability,
            created: true,
        })
    }

    pub(crate) fn cleanup_prepared_reachability(
        &mut self,
        prepared: &PreparedReachability,
    ) -> Result<(), ConnectivityError> {
        if !prepared.created {
            return Ok(());
        }
        self.connection
            .execute(
                "DELETE FROM reachabilities WHERE reachability_id = ?1",
                params![prepared.reachability.id.as_str()],
            )
            .map_err(|error| {
                ConnectivityError::persistence(format!("delete reachability: {error}"))
            })?;
        Ok(())
    }

    pub(crate) fn get_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError> {
        self.connection
            .query_row(
                "
                SELECT reachability_id, ingress_route_id, connectivity_scope
                FROM reachabilities
                WHERE reachability_id = ?1
                ",
                params![reachability_id.as_str()],
                load_reachability_row,
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => ConnectivityError::NotFound,
                other => map_load_error("get reachability", other),
            })
    }

    pub(crate) fn find_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<Option<Reachability>, ConnectivityError> {
        self.connection
            .query_row(
                "
                SELECT reachability_id, ingress_route_id, connectivity_scope
                FROM reachabilities
                WHERE ingress_route_id = ?1
                  AND connectivity_scope = ?2
                ",
                params![ingress_route_id.as_str(), scope.as_str()],
                load_reachability_row,
            )
            .optional()
            .map_err(|error| map_load_error("find reachability", error))
    }

    fn check_integrity(&self) -> Result<(), ConnectivityError> {
        let mut statement = self
            .connection
            .prepare("SELECT reachability_id FROM reachabilities")
            .map_err(|error| {
                ConnectivityError::persistence(format!(
                    "prepare connectivity integrity query: {error}"
                ))
            })?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| {
                ConnectivityError::persistence(format!("query reachability ids: {error}"))
            })?;
        for id in ids {
            let id = id.map_err(|error| {
                ConnectivityError::persistence(format!("read reachability id: {error}"))
            })?;
            let _ = self.get_reachability(&ReachabilityId::parse(id)?)?;
        }
        Ok(())
    }
}

fn load_reachability_row(row: &rusqlite::Row<'_>) -> Result<Reachability, rusqlite::Error> {
    Ok(Reachability {
        id: ReachabilityId::parse(row.get::<_, String>(0)?).map_err(to_sqlite_error)?,
        ingress_route_id: IngressRouteId::parse(row.get::<_, String>(1)?)
            .map_err(|error| to_sqlite_error(ConnectivityError::integrity(error.to_string())))?,
        scope: ConnectivityScope::parse(&row.get::<_, String>(2)?).map_err(to_sqlite_error)?,
    })
}

fn to_sqlite_error(error: ConnectivityError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(error.to_string())),
    )
}

fn map_load_error(operation: &str, error: rusqlite::Error) -> ConnectivityError {
    match error {
        rusqlite::Error::FromSqlConversionFailure(_, _, source) => {
            ConnectivityError::integrity(source.to_string())
        }
        other => ConnectivityError::persistence(format!("failed to {operation}: {other}")),
    }
}
