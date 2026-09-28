use fabric_package_relational_database::{
    RelationalDatabase, RelationalDatabaseError, RelationalDatabaseErrorKind,
    RelationalQueryResult, RelationalRow, RelationalValue,
};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

use crate::config::SqliteDatabasePath;
use crate::conversion::{relational_value, sqlite_values};
use crate::error::sqlite_error;
use crate::state::SqliteDatabaseState;

fabric::adapter! {
    pub SqliteRelationalDatabase for RelationalDatabase {
        id: "onoal.package.data.sqlite.relational-database";

        config {
            path: SqliteDatabasePath;
        }

        state {
            SqliteDatabaseState = SqliteDatabaseState::default();
        }

        runtime {
            async fn execute(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<usize, RelationalDatabaseError> {
                self.state.get().with_connection(
                    RelationalDatabaseErrorKind::ExecuteFailed,
                    |connection| {
                        let parameters = sqlite_values(parameters);
                        connection
                            .execute(&statement, params_from_iter(parameters))
                            .map_err(|error| sqlite_error(RelationalDatabaseErrorKind::ExecuteFailed, error))
                    },
                )
            }

            async fn query(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<RelationalQueryResult, RelationalDatabaseError> {
                self.state.get().with_connection(
                    RelationalDatabaseErrorKind::QueryFailed,
                    |connection| {
                        let parameters = sqlite_values(parameters);
                        let mut statement = connection
                            .prepare(&statement)
                            .map_err(|error| sqlite_error(RelationalDatabaseErrorKind::QueryFailed, error))?;
                        let column_count = statement.column_count();
                        let rows = statement
                            .query_map(params_from_iter(parameters), |row| {
                                let mut values = Vec::with_capacity(column_count);
                                for index in 0..column_count {
                                    let value: Value = row.get(index)?;
                                    values.push(relational_value(value));
                                }
                                Ok(RelationalRow::new(values))
                            })
                            .map_err(|error| sqlite_error(RelationalDatabaseErrorKind::QueryFailed, error))?;

                        let mut collected = Vec::new();
                        for row in rows {
                            collected.push(row.map_err(|error| {
                                sqlite_error(RelationalDatabaseErrorKind::QueryFailed, error)
                            })?);
                        }
                        Ok(RelationalQueryResult::new(collected))
                    },
                )
            }
        }

        lifecycle {
            start {
                let connection = open_connection(&self.config.path)?;
                self.state.get().mark_started(connection)
            }

            stop {
                self.state.get().stop()
            }
        }
    }
}

fn open_connection(path: &SqliteDatabasePath) -> Result<Connection, fabric::core::ModuleError> {
    match path {
        SqliteDatabasePath::File(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    fabric::core::ModuleError::new(format!(
                        "sqlite database directory failed: {error}"
                    ))
                })?;
            }
            Connection::open(path).map_err(|error| {
                fabric::core::ModuleError::new(format!("sqlite open failed: {error}"))
            })
        }
        SqliteDatabasePath::InMemory => Connection::open_in_memory().map_err(|error| {
            fabric::core::ModuleError::new(format!("sqlite in-memory open failed: {error}"))
        }),
    }
}
