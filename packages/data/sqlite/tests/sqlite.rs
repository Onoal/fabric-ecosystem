use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fabric::authoring::ResourceDefinition;
use fabric::component::ComponentRelationName;
use fabric::prelude::*;
use fabric_package_relational_database::{
    RelationalDatabase, RelationalDatabaseError, RelationalDatabaseErrorKind,
    RelationalQueryResult, RelationalValue,
};
use fabric_package_sqlite::{in_memory_sqlite_database, sqlite_database};
use futures::executor::block_on;

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
                resolve_resource(self.relations().database.execute(statement, parameters))
            }

            fn query(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<RelationalQueryResult, RelationalDatabaseError> {
                resolve_resource(self.relations().database.query(statement, parameters))
            }

            fn create_insert_query(&self) -> Result<DatabaseExercise, RelationalDatabaseError> {
                resolve_resource(self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS component_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)".to_owned(),
                    vec![],
                ))?;
                let inserted = resolve_resource(self.relations().database.execute(
                    "INSERT INTO component_items (id, name) VALUES (?1, ?2)".to_owned(),
                    vec![
                        RelationalValue::Integer(1),
                        RelationalValue::Text("component".to_owned()),
                    ],
                ))?;
                let rows = resolve_resource(self.relations().database.query(
                    "SELECT id, name FROM component_items ORDER BY id".to_owned(),
                    vec![],
                ))?;
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
            fn write_and_read_both(&self) -> Result<(RelationalQueryResult, RelationalQueryResult), RelationalDatabaseError>;
        }

        runtime {
            fn write_and_read_both(&self) -> Result<(RelationalQueryResult, RelationalQueryResult), RelationalDatabaseError> {
                resolve_resource(self.relations().primary.execute(
                    "CREATE TABLE marker (value TEXT NOT NULL)".to_owned(),
                    vec![],
                ))?;
                resolve_resource(self.relations().secondary.execute(
                    "CREATE TABLE marker (value TEXT NOT NULL)".to_owned(),
                    vec![],
                ))?;
                resolve_resource(self.relations().primary.execute(
                    "INSERT INTO marker (value) VALUES (?1)".to_owned(),
                    vec![RelationalValue::Text("primary".to_owned())],
                ))?;
                resolve_resource(self.relations().secondary.execute(
                    "INSERT INTO marker (value) VALUES (?1)".to_owned(),
                    vec![RelationalValue::Text("secondary".to_owned())],
                ))?;
                Ok((
                    resolve_resource(self.relations()
                            .primary
                            .query("SELECT value FROM marker".to_owned(), vec![]),
                    )?,
                    resolve_resource(self.relations()
                            .secondary
                            .query("SELECT value FROM marker".to_owned(), vec![]),
                    )?,
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

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("sqlite test resource operation unexpectedly yielded")
        }
    }
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

fn file_composition(id: &str, name: &'static str, path: PathBuf) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(sqlite_database(name, path))
        .with(probe_for(name))
        .build()
        .expect("composition")
}

fn memory_composition(id: &str, name: &'static str) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(in_memory_sqlite_database(name))
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
    let composition = file_composition("onoal.package.test.sqlite.basic", "primary", path.clone());
    let resource = composition.resources().next().expect("database resource");
    assert_eq!(resource.resource_id(), &RelationalDatabase::resource_id());
    assert_eq!(resource.name().as_str(), "primary");
    assert_eq!(composition.components().count(), 1);
    let instance = started_instance(&composition, "onoal.package.test.sqlite.basic.instance");
    let database = probe(&instance);

    block_on(
        database.execute(
            "CREATE TABLE items (id INTEGER, score REAL, label TEXT, payload BLOB, optional TEXT)"
                .to_owned(),
            vec![],
        ),
    )
    .expect("create")
    .expect("create ok");
    block_on(
        database.execute(
            "INSERT INTO items (id, score, label, payload, optional) VALUES (?1, ?2, ?3, ?4, ?5)"
                .to_owned(),
            vec![
                RelationalValue::Integer(1),
                RelationalValue::Real(42.5),
                RelationalValue::Text("alpha".to_owned()),
                RelationalValue::Bytes(vec![1, 2, 3]),
                RelationalValue::Null,
            ],
        ),
    )
    .expect("insert one")
    .expect("insert one ok");
    block_on(
        database.execute(
            "INSERT INTO items (id, score, label, payload, optional) VALUES (?1, ?2, ?3, ?4, ?5)"
                .to_owned(),
            vec![
                RelationalValue::Integer(2),
                RelationalValue::Real(7.0),
                RelationalValue::Text("beta".to_owned()),
                RelationalValue::Bytes(vec![9]),
                RelationalValue::Text("present".to_owned()),
            ],
        ),
    )
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
fn in_memory_sqlite_executes_real_create_insert_query() {
    let composition = memory_composition("onoal.package.test.sqlite.memory", "primary");
    let instance = started_instance(&composition, "onoal.package.test.sqlite.memory.instance");
    let database = probe(&instance);

    let exercise = block_on(database.create_insert_query())
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
    let composition = file_composition(
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
fn in_memory_generation_starts_empty() {
    let composition = memory_composition("onoal.package.test.sqlite.memory-generation", "primary");
    {
        let mut first = started_instance(
            &composition,
            "onoal.package.test.sqlite.memory-generation.first",
        );
        let database = probe(&first);
        block_on(database.execute(
            "CREATE TABLE generation_marker (value TEXT NOT NULL)".to_owned(),
            vec![],
        ))
        .expect("create")
        .expect("create ok");
        block_on(database.execute(
            "INSERT INTO generation_marker (value) VALUES (?1)".to_owned(),
            vec![RelationalValue::Text("temporary".to_owned())],
        ))
        .expect("insert")
        .expect("insert ok");
        first.stop().expect("stop first");
    }
    {
        let second = started_instance(
            &composition,
            "onoal.package.test.sqlite.memory-generation.second",
        );
        let database = probe(&second);
        let error =
            block_on(database.query("SELECT value FROM generation_marker".to_owned(), vec![]))
                .expect("query call")
                .expect_err("fresh memory database has no old table");
        assert_eq!(error.kind, RelationalDatabaseErrorKind::QueryFailed);
    }
}

#[test]
fn invalid_sql_crosses_package_error_boundary() {
    let path = unique_path("error");
    let composition = file_composition("onoal.package.test.sqlite.error", "primary", path.clone());
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
fn stopped_generation_returns_bounded_lifecycle_error_when_reachable() {
    let composition = memory_composition("onoal.package.test.sqlite.lifecycle", "primary");
    let mut instance =
        started_instance(&composition, "onoal.package.test.sqlite.lifecycle.instance");
    {
        let database = probe(&instance);
        let _ = block_on(database.execute(
            "CREATE TABLE lifecycle (value TEXT NOT NULL)".to_owned(),
            vec![],
        ))
        .expect("execute")
        .expect("execute ok");
    }
    instance.stop().expect("stop");

    if let Ok(database) = instance.component::<SqliteDatabaseProbe>() {
        let result = block_on(database.execute(
            "INSERT INTO lifecycle (value) VALUES ('after-stop')".to_owned(),
            vec![],
        ));
        match result {
            Ok(Err(error)) => assert_eq!(error.kind, RelationalDatabaseErrorKind::Stopped),
            Err(_) => {}
            Ok(Ok(count)) => panic!("stopped sqlite runtime fabricated success: {count}"),
        }
    }
}
