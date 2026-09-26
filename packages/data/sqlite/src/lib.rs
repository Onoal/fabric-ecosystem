//! SQLite realization package for Fabric RelationalDatabase.
//!
//! The RelationalDatabase Resource is owned by
//! `onoal-fabric-package-relational-database`. This crate supplies a concrete
//! SQLite Adapter and authoring helpers.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use fabric::prelude::*;
use fabric_package_relational_database::{
    RelationalDatabase, RelationalDatabaseError, RelationalDatabaseErrorKind,
    RelationalQueryResult, RelationalRow, RelationalValue,
};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqliteDatabasePath {
    File(PathBuf),
    InMemory,
}

#[derive(Default)]
pub struct SqliteDatabaseState {
    connection: Mutex<Option<Connection>>,
    live: AtomicBool,
    stopped: AtomicBool,
}

impl SqliteDatabaseState {
    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, RelationalDatabaseError>,
    ) -> Result<T, RelationalDatabaseError> {
        if !self.live.load(Ordering::SeqCst) {
            return if self.stopped.load(Ordering::SeqCst) {
                Err(RelationalDatabaseError::stopped())
            } else {
                Err(RelationalDatabaseError::not_started())
            };
        }
        let guard = self.connection.lock().expect("sqlite connection");
        let connection = guard
            .as_ref()
            .ok_or_else(RelationalDatabaseError::not_started)?;
        operation(connection)
    }
}

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
            fn execute(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<usize, RelationalDatabaseError> {
                self.state.get().with_connection(|connection| {
                    let parameters = sqlite_values(parameters);
                    connection
                        .execute(&statement, params_from_iter(parameters))
                        .map_err(|error| sqlite_error(RelationalDatabaseErrorKind::ExecuteFailed, error))
                })
            }

            fn query(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<RelationalQueryResult, RelationalDatabaseError> {
                self.state.get().with_connection(|connection| {
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
                })
            }
        }

        lifecycle {
            start {
                let connection = match &self.config.path {
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
                        })?
                    }
                    SqliteDatabasePath::InMemory => Connection::open_in_memory().map_err(|error| {
                        fabric::core::ModuleError::new(format!("sqlite in-memory open failed: {error}"))
                    })?,
                };
                *self.state.get().connection.lock().expect("sqlite connection") = Some(connection);
                self.state.get().stopped.store(false, Ordering::SeqCst);
                self.state.get().live.store(true, Ordering::SeqCst);
                Ok(())
            }

            stop {
                self.state.get().live.store(false, Ordering::SeqCst);
                self.state.get().stopped.store(true, Ordering::SeqCst);
                let connection = self
                    .state
                    .get()
                    .connection
                    .lock()
                    .expect("sqlite connection")
                    .take();
                if let Some(connection) = connection {
                    connection.close().map_err(|(_, error)| {
                        fabric::core::ModuleError::new(format!("sqlite close failed: {error}"))
                    })?;
                }
                Ok(())
            }
        }
    }
}

fn sqlite_values(values: Vec<RelationalValue>) -> Vec<Value> {
    values
        .into_iter()
        .map(|value| match value {
            RelationalValue::Null => Value::Null,
            RelationalValue::Integer(value) => Value::Integer(value),
            RelationalValue::Real(value) => Value::Real(value),
            RelationalValue::Text(value) => Value::Text(value),
            RelationalValue::Bytes(value) => Value::Blob(value),
        })
        .collect()
}

fn relational_value(value: Value) -> RelationalValue {
    match value {
        Value::Null => RelationalValue::Null,
        Value::Integer(value) => RelationalValue::Integer(value),
        Value::Real(value) => RelationalValue::Real(value),
        Value::Text(value) => RelationalValue::Text(value),
        Value::Blob(value) => RelationalValue::Bytes(value),
    }
}

fn sqlite_error(
    kind: RelationalDatabaseErrorKind,
    error: rusqlite::Error,
) -> RelationalDatabaseError {
    RelationalDatabaseError::new(kind, error.to_string())
}

pub fn sqlite_database(
    name: &'static str,
    path: impl Into<PathBuf>,
) -> impl IntoFabricContribution {
    sqlite_database_with(name, SqliteDatabasePath::File(path.into()))
}

pub fn in_memory_sqlite_database(name: &'static str) -> impl IntoFabricContribution {
    sqlite_database_with(name, SqliteDatabasePath::InMemory)
}

pub fn sqlite_database_with(
    name: &'static str,
    path: SqliteDatabasePath,
) -> impl IntoFabricContribution {
    let selected = RelationalDatabase::select(name).expect("valid relational database name");
    FabricContribution::new().resource(
        selected
            .using(SqliteRelationalDatabase::new(
                SqliteRelationalDatabaseConfig { path },
            ))
            .expect("SqliteRelationalDatabase supports RelationalDatabase"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric::authoring::ResourceDefinition;
    use fabric::component::ComponentRelationName;
    use futures::executor::block_on;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone, Debug, PartialEq)]
    pub struct DatabaseExercise {
        pub inserted: usize,
        pub rows: RelationalQueryResult,
    }

    fabric::component! {
        SqliteDatabaseProbe {
            id: "onoal.package.data.sqlite.test-probe";

            relations {
                requires {
                    database: RelationalDatabase;
                }
            }

            api {
                fn execute(
                    &self,
                    statement: String,
                    parameters: Vec<RelationalValue>,
                ) -> Result<usize, RelationalDatabaseError>;

                fn query(
                    &self,
                    statement: String,
                    parameters: Vec<RelationalValue>,
                ) -> Result<RelationalQueryResult, RelationalDatabaseError>;

                fn create_insert_query(&self) -> Result<DatabaseExercise, RelationalDatabaseError>;
            }

            runtime {
                fn execute(
                    &self,
                    statement: String,
                    parameters: Vec<RelationalValue>,
                ) -> Result<usize, RelationalDatabaseError> {
                    self.relations().database.execute(statement, parameters)
                }

                fn query(
                    &self,
                    statement: String,
                    parameters: Vec<RelationalValue>,
                ) -> Result<RelationalQueryResult, RelationalDatabaseError> {
                    self.relations().database.query(statement, parameters)
                }

                fn create_insert_query(&self) -> Result<DatabaseExercise, RelationalDatabaseError> {
                    self.relations().database.execute(
                        "CREATE TABLE IF NOT EXISTS component_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_owned(),
                        vec![],
                    )?;
                    let inserted = self.relations().database.execute(
                        "INSERT INTO component_items (id, name) VALUES (?1, ?2)".to_owned(),
                        vec![
                            RelationalValue::Integer(1),
                            RelationalValue::Text("component".to_owned()),
                        ],
                    )?;
                    let rows = self.relations().database.query(
                        "SELECT id, name FROM component_items ORDER BY id".to_owned(),
                        vec![],
                    )?;
                    Ok(DatabaseExercise { inserted, rows })
                }
            }
        }
    }

    fabric::component! {
        DualSqliteDatabaseProbe {
            id: "onoal.package.data.sqlite.test-dual-probe";

            relations {
                requires {
                    primary: RelationalDatabase;
                    secondary: RelationalDatabase;
                }
            }

            api {
                fn write_and_read_both(
                    &self,
                ) -> Result<(RelationalQueryResult, RelationalQueryResult), RelationalDatabaseError>;
            }

            runtime {
                fn write_and_read_both(
                    &self,
                ) -> Result<(RelationalQueryResult, RelationalQueryResult), RelationalDatabaseError> {
                    self.relations().primary.execute(
                        "CREATE TABLE marker (value TEXT NOT NULL)".to_owned(),
                        vec![],
                    )?;
                    self.relations().secondary.execute(
                        "CREATE TABLE marker (value TEXT NOT NULL)".to_owned(),
                        vec![],
                    )?;
                    self.relations().primary.execute(
                        "INSERT INTO marker (value) VALUES (?1)".to_owned(),
                        vec![RelationalValue::Text("primary".to_owned())],
                    )?;
                    self.relations().secondary.execute(
                        "INSERT INTO marker (value) VALUES (?1)".to_owned(),
                        vec![RelationalValue::Text("secondary".to_owned())],
                    )?;
                    Ok((
                        self.relations()
                            .primary
                            .query("SELECT value FROM marker".to_owned(), vec![])?,
                        self.relations()
                            .secondary
                            .query("SELECT value FROM marker".to_owned(), vec![])?,
                    ))
                }
            }
        }
    }

    fn unique_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "onoal-fabric-sqlite-{label}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    fn probe_for(name: &'static str) -> impl IntoFabricContribution {
        let selected = RelationalDatabase::select(name).expect("database selection");
        let component = SqliteDatabaseProbe::define().select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                ComponentRelationName::new("database").expect("role"),
                fabric::authoring::Requires::<RelationalDatabase>::provisional(),
            ),
            &selected,
        );
        FabricContribution::new().component(component)
    }

    fn dual_probe_component() -> impl IntoFabricContribution {
        let primary = RelationalDatabase::select("primary").expect("primary selection");
        let secondary = RelationalDatabase::select("secondary").expect("secondary selection");
        let component = DualSqliteDatabaseProbe::define()
            .select_named_resource_provider(
                &fabric::authoring::ComponentResourceRequirement::new(
                    ComponentRelationName::new("primary").expect("role"),
                    fabric::authoring::Requires::<RelationalDatabase>::provisional(),
                ),
                &primary,
            )
            .select_named_resource_provider(
                &fabric::authoring::ComponentResourceRequirement::new(
                    ComponentRelationName::new("secondary").expect("role"),
                    fabric::authoring::Requires::<RelationalDatabase>::provisional(),
                ),
                &secondary,
            );
        FabricContribution::new().component(component)
    }

    fn composition(id: &str, name: &'static str, path: PathBuf) -> Composition {
        Fabric::new(id)
            .expect("fabric")
            .with(sqlite_database(name, path))
            .with(probe_for(name))
            .build()
            .expect("composition")
    }

    fn started_instance(composition: &Composition, id: &str) -> Instance {
        let mut instance = composition
            .materialize_on(id, &HostDescriptor::native())
            .expect("instance");
        instance.start().expect("start");
        instance
    }

    fn probe(instance: &Instance) -> BoundComponent<'_, SqliteDatabaseProbe> {
        let component = instance
            .component::<SqliteDatabaseProbe>()
            .expect("database probe");
        component.reconcile().expect("reconcile database probe");
        component
    }

    fn dual_probe(instance: &Instance) -> BoundComponent<'_, DualSqliteDatabaseProbe> {
        let component = instance
            .component::<DualSqliteDatabaseProbe>()
            .expect("dual database probe");
        component
            .reconcile()
            .expect("reconcile dual database probe");
        component
    }

    #[test]
    fn execute_query_parameters_rows_and_value_kinds_use_real_sqlite() {
        let path = unique_path("basic");
        let composition = composition("onoal.package.test.sqlite.basic", "primary", path.clone());
        let resource = composition.resources().next().expect("database resource");
        assert_eq!(resource.resource_id(), &RelationalDatabase::resource_id());
        assert_eq!(resource.name().as_str(), "primary");
        assert_eq!(composition.components().count(), 1);
        let instance = started_instance(&composition, "onoal.package.test.sqlite.basic.instance");
        let database = probe(&instance);

        block_on(database.execute(
            "CREATE TABLE items (id INTEGER, score REAL, label TEXT, payload BLOB, optional TEXT)".to_owned(),
            vec![],
        ))
        .expect("create")
        .expect("create ok");
        block_on(database.execute(
            "INSERT INTO items (id, score, label, payload, optional) VALUES (?1, ?2, ?3, ?4, ?5)".to_owned(),
            vec![
                RelationalValue::Integer(1),
                RelationalValue::Real(42.5),
                RelationalValue::Text("alpha".to_owned()),
                RelationalValue::Bytes(vec![1, 2, 3]),
                RelationalValue::Null,
            ],
        ))
        .expect("insert one")
        .expect("insert one ok");
        block_on(database.execute(
            "INSERT INTO items (id, score, label, payload, optional) VALUES (?1, ?2, ?3, ?4, ?5)".to_owned(),
            vec![
                RelationalValue::Integer(2),
                RelationalValue::Real(7.0),
                RelationalValue::Text("beta".to_owned()),
                RelationalValue::Bytes(vec![9]),
                RelationalValue::Text("present".to_owned()),
            ],
        ))
        .expect("insert two")
        .expect("insert two ok");

        let rows = block_on(
            database.query(
                "SELECT id, score, label, payload, optional FROM items WHERE id >= ?1 ORDER BY id"
                    .to_owned(),
                vec![RelationalValue::Integer(1)],
            ),
        )
        .expect("query")
        .expect("query ok");
        assert_eq!(rows.rows().len(), 2);
        assert_eq!(
            rows.rows()[0].values(),
            &[
                RelationalValue::Integer(1),
                RelationalValue::Real(42.5),
                RelationalValue::Text("alpha".to_owned()),
                RelationalValue::Bytes(vec![1, 2, 3]),
                RelationalValue::Null,
            ]
        );
        assert_eq!(
            rows.rows()[1].get(2),
            Some(&RelationalValue::Text("beta".to_owned()))
        );
        drop(instance);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn named_occurrences_use_separate_sqlite_files() {
        let primary_path = unique_path("primary");
        let secondary_path = unique_path("secondary");
        let composition = Fabric::new("onoal.package.test.sqlite.occurrences")
            .expect("fabric")
            .with(sqlite_database("primary", primary_path.clone()))
            .with(sqlite_database("secondary", secondary_path.clone()))
            .with(dual_probe_component())
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.package.test.sqlite.occurrences.instance",
        );
        let (primary_rows, secondary_rows) = block_on(dual_probe(&instance).write_and_read_both())
            .expect("dual probe call")
            .expect("dual probe database ok");
        assert_eq!(
            primary_rows.rows()[0].get(0),
            Some(&RelationalValue::Text("primary".to_owned()))
        );
        assert_eq!(
            secondary_rows.rows()[0].get(0),
            Some(&RelationalValue::Text("secondary".to_owned()))
        );
        drop(instance);
        let _ = std::fs::remove_file(primary_path);
        let _ = std::fs::remove_file(secondary_path);
    }

    #[test]
    fn file_backed_data_survives_fresh_fabric_generation_and_reopens_cleanly() {
        let path = unique_path("persistent");
        let composition = composition(
            "onoal.package.test.sqlite.persistence",
            "primary",
            path.clone(),
        );
        {
            let mut first = started_instance(
                &composition,
                "onoal.package.test.sqlite.persistence.generation-a",
            );
            let database = probe(&first);
            block_on(database.execute(
                "CREATE TABLE durable (value TEXT NOT NULL)".to_owned(),
                vec![],
            ))
            .expect("create")
            .expect("create ok");
            block_on(database.execute(
                "INSERT INTO durable (value) VALUES (?1)".to_owned(),
                vec![RelationalValue::Text("survived".to_owned())],
            ))
            .expect("insert")
            .expect("insert ok");
            first.stop().expect("stop first");
        }
        {
            let second = started_instance(
                &composition,
                "onoal.package.test.sqlite.persistence.generation-b",
            );
            let database = probe(&second);
            let rows = block_on(database.query("SELECT value FROM durable".to_owned(), vec![]))
                .expect("query")
                .expect("query ok");
            assert_eq!(
                rows.rows()[0].get(0),
                Some(&RelationalValue::Text("survived".to_owned()))
            );
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_sql_crosses_package_error_boundary() {
        let path = unique_path("error");
        let composition = composition("onoal.package.test.sqlite.error", "primary", path.clone());
        let instance = started_instance(&composition, "onoal.package.test.sqlite.error.instance");
        let database = probe(&instance);

        let error = block_on(database.execute(
            "INSERT INTO missing_table (value) VALUES (?1)".to_owned(),
            vec![RelationalValue::Text("bad".to_owned())],
        ))
        .expect("execute call")
        .expect_err("database error");
        assert_eq!(error.kind, RelationalDatabaseErrorKind::ExecuteFailed);
        assert!(!error.detail.contains("rusqlite::"));
        drop(instance);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn component_can_consume_relational_database_normally() {
        let path = unique_path("component");
        let selected = RelationalDatabase::select("primary").expect("database selection");
        let client = SqliteDatabaseProbe::define().select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                ComponentRelationName::new("database").expect("role"),
                fabric::authoring::Requires::<RelationalDatabase>::provisional(),
            ),
            &selected,
        );
        let composition = Fabric::new("onoal.package.test.sqlite.component")
            .expect("fabric")
            .with(sqlite_database("primary", path.clone()))
            .component(client)
            .build()
            .expect("composition");
        let instance =
            started_instance(&composition, "onoal.package.test.sqlite.component.instance");
        let client = instance.component::<SqliteDatabaseProbe>().expect("client");
        client.reconcile().expect("reconcile");
        let exercise = block_on(client.create_insert_query())
            .expect("component call")
            .expect("component db ok");
        assert_eq!(exercise.inserted, 1);
        assert_eq!(
            exercise.rows.rows()[0].values(),
            &[
                RelationalValue::Integer(1),
                RelationalValue::Text("component".to_owned()),
            ]
        );
        drop(instance);
        let _ = std::fs::remove_file(path);
    }
}
