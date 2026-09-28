use std::error::Error;
use std::fmt;

use fabric::prelude::*;
use fabric_package_observability_logging::{LogError, LogLevel, LogRecord, LogSink};
use fabric_package_relational_database::{
    RelationalDatabase, RelationalDatabaseError, RelationalValue,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoredData {
    pub(crate) value: String,
    pub(crate) logged: LogRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExampleBackendError {
    Database(RelationalDatabaseError),
    Log(LogError),
    MissingRow,
    UnexpectedValue,
}

impl fmt::Display for ExampleBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database operation failed: {error}"),
            Self::Log(error) => write!(f, "log emission failed: {error}"),
            Self::MissingRow => f.write_str("example row was missing"),
            Self::UnexpectedValue => f.write_str("example row contained an unexpected value type"),
        }
    }
}

impl Error for ExampleBackendError {}

impl From<RelationalDatabaseError> for ExampleBackendError {
    fn from(error: RelationalDatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<LogError> for ExampleBackendError {
    fn from(error: LogError) -> Self {
        Self::Log(error)
    }
}

fabric::component! {
    pub ExampleBackendApp {
        id: "onoal.ecosystem.example.local-backend.app";

        relations {
            requires {
                database: RelationalDatabase;
                log: LogSink;
            }
        }

        api {
            fn write_read_log(&self, value: String) -> Result<StoredData, ExampleBackendError>;
        }

        runtime {
            fn write_read_log(&self, value: String) -> Result<StoredData, ExampleBackendError> {
                resolve_resource(self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS example_items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)".to_owned(),
                    vec![],
                ))?;
                resolve_resource(self.relations().database.execute(
                    "INSERT INTO example_items (id, value) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET value = excluded.value".to_owned(),
                    vec![RelationalValue::Integer(1), RelationalValue::Text(value)],
                ))?;
                let rows = resolve_resource(self.relations().database.query(
                    "SELECT value FROM example_items WHERE id = ?1".to_owned(),
                    vec![RelationalValue::Integer(1)],
                ))?;
                let stored = match rows.rows().first().and_then(|row| row.get(0)) {
                    Some(RelationalValue::Text(value)) => value.clone(),
                    Some(_) => return Err(ExampleBackendError::UnexpectedValue),
                    None => return Err(ExampleBackendError::MissingRow),
                };
                let logged = LogRecord::targeted(
                    LogLevel::Info,
                    "local-backend-example",
                    "example stored local data",
                );
                resolve_resource(self.relations().log.emit(logged.clone()))?;
                Ok(StoredData {
                    value: stored,
                    logged,
                })
            }
        }
    }
}

pub(crate) fn backend_app(
    database_name: &'static str,
    log_name: &'static str,
) -> impl IntoFabricContribution {
    let database =
        RelationalDatabase::select(database_name).expect("valid relational database resource name");
    let log = LogSink::select(log_name).expect("valid log sink resource name");
    let component = ExampleBackendApp::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("database").expect("role"),
                fabric::authoring::Requires::<RelationalDatabase>::provisional(),
            ),
            &database,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("log").expect("role"),
                fabric::authoring::Requires::<LogSink>::provisional(),
            ),
            &log,
        );
    FabricContribution::new().component(component)
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("local backend resource operation unexpectedly yielded")
        }
    }
}
