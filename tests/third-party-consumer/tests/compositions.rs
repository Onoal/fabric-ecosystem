mod support;

use fabric::prelude::*;
use fabric_composition_http_server::{http_server_stack, HttpServerCompositionConfig};
use fabric_composition_local_backend::{local_backend_stack, LocalBackendCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{TcpTransportInspector, TcpTransportInspectorInstanceApi};
use fabric_package_observability_logging::{LogError, LogRecord, LogSink};
use fabric_package_relational_database::{
    RelationalDatabase, RelationalDatabaseError, RelationalQueryResult, RelationalValue,
};
use futures::executor::block_on;
use support::fabric::{activate, started_instance};
use support::http_client::http_client;
use support::temp_database::TempDatabase;

#[derive(Clone, Debug, PartialEq)]
struct BackendConsumerResult {
    rows: RelationalQueryResult,
    logged: LogRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BackendConsumerError {
    Database(RelationalDatabaseError),
    Log(LogError),
}

impl From<RelationalDatabaseError> for BackendConsumerError {
    fn from(error: RelationalDatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<LogError> for BackendConsumerError {
    fn from(error: LogError) -> Self {
        Self::Log(error)
    }
}

fabric::component! {
    BackendConsumer {
        id: "onoal.test.third-party.compositions.backend-consumer";

        relations {
            requires {
                database: RelationalDatabase;
                log: LogSink;
            }
        }

        api {
            fn exercise_backend(&self) -> Result<BackendConsumerResult, BackendConsumerError>;
        }

        runtime {
            fn exercise_backend(&self) -> Result<BackendConsumerResult, BackendConsumerError> {
                self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS third_party_backend_items (id INTEGER, value TEXT NOT NULL)".to_owned(),
                    vec![],
                )?;
                self.relations().database.execute(
                    "INSERT INTO third_party_backend_items (id, value) VALUES (?1, ?2)".to_owned(),
                    vec![
                        RelationalValue::Integer(1),
                        RelationalValue::Text("backend-consumer".to_owned()),
                    ],
                )?;
                let rows = self.relations().database.query(
                    "SELECT id, value FROM third_party_backend_items ORDER BY id".to_owned(),
                    vec![],
                )?;
                let logged = LogRecord::targeted(
                    fabric_package_observability_logging::LogLevel::Info,
                    "third-party-local-backend",
                    "third-party backend app used local backend foundation",
                );
                self.relations().log.emit(logged.clone())?;
                Ok(BackendConsumerResult { rows, logged })
            }
        }
    }
}

fn backend_consumer(
    database_name: &'static str,
    log_name: &'static str,
) -> impl IntoFabricContribution {
    let database = RelationalDatabase::select(database_name).expect("database selection");
    let log = LogSink::select(log_name).expect("log selection");
    let component = BackendConsumer::define()
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

#[test]
fn external_consumer_uses_http_server_composition_for_real_exchange() {
    let composition = Fabric::new("onoal.test.third-party.compositions.http")
        .expect("fabric")
        .with(http_server_stack(HttpServerCompositionConfig::local("api")))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.compositions.http.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = block_on(inspector.inspect_transport())
        .expect("observe transport")
        .actual
        .expect("bound TCP address");
    let client = http_client(address, "/third-party");

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    let request = exchange.request().clone();
    exchange
        .respond(HttpResponse::new(
            200,
            format!("third-party:{}", request.target).into_bytes(),
        ))
        .expect("respond");
    let response = client.join().expect("client");

    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/third-party");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.ends_with("\r\n\r\nthird-party:/third-party"));
}

#[test]
fn external_consumer_reuses_http_stack_inside_larger_fabric_build() {
    let composition = Fabric::new("onoal.test.third-party.compositions.http.reuse")
        .expect("fabric")
        .with(http_server_stack(HttpServerCompositionConfig::local("api")))
        .with(fabric_package_key_value::memory_key_value("scratch"))
        .build()
        .expect("composition");

    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "api"));
    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "scratch"));
    assert!(composition.components().any(|component| {
        component.component_id().as_str() == "onoal.package.networking.http.server"
    }));
}

#[test]
fn external_consumer_extends_local_backend_composition_with_own_app() {
    let database = TempDatabase::new("local-backend");
    let composition = Fabric::new("onoal.test.third-party.compositions.local-backend")
        .expect("fabric")
        .with(local_backend_stack(LocalBackendCompositionConfig::local(
            "api",
            "primary-db",
            database.path_buf(),
            "application-log",
        )))
        .with(backend_consumer("primary-db", "application-log"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.compositions.local-backend.instance",
    );
    let app = activate::<BackendConsumer>(&instance);

    let output = block_on(app.exercise_backend())
        .expect("exercise")
        .expect("backend result");

    assert_eq!(
        output.rows.rows()[0].values(),
        &[
            RelationalValue::Integer(1),
            RelationalValue::Text("backend-consumer".to_owned()),
        ]
    );
    assert_eq!(
        output.logged.target.as_deref(),
        Some("third-party-local-backend")
    );
}
