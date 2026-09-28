use fabric::prelude::*;
use fabric_package_observability_logging::{LogRecord, LogSink};
use fabric_package_relational_database::{
    RelationalDatabase, RelationalQueryResult, RelationalValue,
};

#[derive(Clone, Debug, PartialEq)]
pub struct TestBackendAppResult {
    pub value: Option<String>,
    pub logged: LogRecord,
}

fabric::component! {
    pub TestLocalBackendApp {
        id: "onoal.composition.test.local-backend.app";

        relations {
            requires {
                database: RelationalDatabase;
                log: LogSink;
            }
        }

        api {
            fn write_read_log(&self, value: String) -> Result<TestBackendAppResult, String>;
            fn read_value(&self) -> Result<Option<String>, String>;
        }

        runtime {
            fn write_read_log(&self, value: String) -> Result<TestBackendAppResult, String> {
                resolve_resource(self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS local_backend_items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)".to_owned(),
                    vec![],
                )).map_err(|error| error.to_string())?;
                resolve_resource(self.relations().database.execute(
                    "INSERT INTO local_backend_items (id, value) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET value = excluded.value".to_owned(),
                    vec![RelationalValue::Integer(1), RelationalValue::Text(value)],
                )).map_err(|error| error.to_string())?;
                let value = query_value(resolve_resource(self.relations().database.query(
                    "SELECT value FROM local_backend_items WHERE id = ?1".to_owned(),
                    vec![RelationalValue::Integer(1)],
                )).map_err(|error| error.to_string())?);
                let logged = LogRecord::targeted(
                    fabric_package_observability_logging::LogLevel::Info,
                    "local-backend-test",
                    "local backend app used database and log",
                );
                resolve_resource(self.relations().log.emit(logged.clone())).map_err(|error| error.to_string())?;
                Ok(TestBackendAppResult { value, logged })
            }

            fn read_value(&self) -> Result<Option<String>, String> {
                let rows = resolve_resource(self.relations().database.query(
                    "SELECT value FROM local_backend_items WHERE id = ?1".to_owned(),
                    vec![RelationalValue::Integer(1)],
                )).map_err(|error| error.to_string())?;
                Ok(query_value(rows))
            }
        }
    }
}

fn query_value(rows: RelationalQueryResult) -> Option<String> {
    rows.rows()
        .first()
        .and_then(|row| row.get(0))
        .and_then(|value| match value {
            RelationalValue::Text(value) => Some(value.clone()),
            _ => None,
        })
}

pub fn test_app(
    database_name: &'static str,
    log_name: &'static str,
) -> impl IntoFabricContribution {
    let database = RelationalDatabase::select(database_name).expect("database selection");
    let log = LogSink::select(log_name).expect("log selection");
    let component = TestLocalBackendApp::define()
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
            panic!("local backend test resource operation unexpectedly yielded")
        }
    }
}
