//! Reusable local backend foundation Composition artifact.
//!
//! This crate composes existing Fabric Ecosystem artifacts. It defines no new
//! production Fabric Resource, System, Component, or Adapter.

use std::path::PathBuf;

use fabric::prelude::*;
use fabric_composition_http_server::{http_server_stack, HttpServerCompositionConfig};
use fabric_package_observability_logging::ConsoleLogConfig;
use fabric_package_sqlite::SqliteDatabasePath;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBackendCompositionConfig {
    pub http: HttpServerCompositionConfig,
    pub database_name: &'static str,
    pub database: SqliteDatabasePath,
    pub log_name: &'static str,
    pub logging: ConsoleLogConfig,
}

impl LocalBackendCompositionConfig {
    pub fn local(
        transport_name: &'static str,
        database_name: &'static str,
        database_path: impl Into<PathBuf>,
        log_name: &'static str,
    ) -> Self {
        Self {
            http: HttpServerCompositionConfig::local(transport_name),
            database_name,
            database: SqliteDatabasePath::File(database_path.into()),
            log_name,
            logging: ConsoleLogConfig::default(),
        }
    }

    pub fn in_memory(
        transport_name: &'static str,
        database_name: &'static str,
        log_name: &'static str,
    ) -> Self {
        Self {
            http: HttpServerCompositionConfig::local(transport_name),
            database_name,
            database: SqliteDatabasePath::InMemory,
            log_name,
            logging: ConsoleLogConfig::default(),
        }
    }
}

pub fn local_backend_stack(config: LocalBackendCompositionConfig) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(http_server_stack(config.http))
        .with(fabric_package_sqlite::sqlite_database_with(
            config.database_name,
            config.database,
        ))
        .with(fabric_package_observability_logging::console_logging_with(
            config.log_name,
            config.logging,
        ))
}

pub fn build_local_backend_composition(
    id: &str,
    config: LocalBackendCompositionConfig,
) -> Result<Composition, Box<dyn std::error::Error>> {
    Fabric::new(id)
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?
        .with(local_backend_stack(config))
        .build()
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
    use fabric_package_networking_tcp::{
        TcpSocketAddress, TcpTransportInspection, TcpTransportInspector,
        TcpTransportInspectorInstanceApi,
    };
    use fabric_package_observability_logging::{LogRecord, LogSink};
    use fabric_package_relational_database::{
        RelationalDatabase, RelationalQueryResult, RelationalValue,
    };
    use futures::executor::block_on;
    use std::io::{Read, Write};
    use std::net::{Shutdown, SocketAddr, TcpStream};
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone, Debug, PartialEq)]
    struct TestBackendAppResult {
        value: Option<String>,
        logged: LogRecord,
    }

    fabric::component! {
        TestLocalBackendApp {
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
                    self.relations().database.execute(
                        "CREATE TABLE IF NOT EXISTS local_backend_items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)".to_owned(),
                        vec![],
                    ).map_err(|error| error.to_string())?;
                    self.relations().database.execute(
                        "INSERT INTO local_backend_items (id, value) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET value = excluded.value".to_owned(),
                        vec![RelationalValue::Integer(1), RelationalValue::Text(value)],
                    ).map_err(|error| error.to_string())?;
                    let value = query_value(self.relations().database.query(
                        "SELECT value FROM local_backend_items WHERE id = ?1".to_owned(),
                        vec![RelationalValue::Integer(1)],
                    ).map_err(|error| error.to_string())?);
                    let logged = LogRecord::targeted(
                        fabric_package_observability_logging::LogLevel::Info,
                        "local-backend-test",
                        "local backend app used database and log",
                    );
                    self.relations().log.emit(logged.clone()).map_err(|error| error.to_string())?;
                    Ok(TestBackendAppResult { value, logged })
                }

                fn read_value(&self) -> Result<Option<String>, String> {
                    let rows = self.relations().database.query(
                        "SELECT value FROM local_backend_items WHERE id = ?1".to_owned(),
                        vec![RelationalValue::Integer(1)],
                    ).map_err(|error| error.to_string())?;
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

    fn test_app(
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

    fn unique_db_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "onoal-fabric-local-backend-{label}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    fn started_instance(composition: &Composition, id: &str) -> Instance {
        let mut instance = composition
            .materialize_on(id, &HostDescriptor::native())
            .expect("instance");
        instance.start().expect("start");
        instance
    }

    fn activate<C: fabric::authoring::ComponentDefinition>(
        instance: &Instance,
    ) -> BoundComponent<'_, C> {
        let component = instance.component::<C>().expect("component");
        component.reconcile().expect("reconcile");
        component
    }

    fn actual_address(inspector: &BoundComponent<'_, TcpTransportInspector>) -> TcpSocketAddress {
        let observation: TcpTransportInspection =
            block_on(inspector.inspect_transport()).expect("observe");
        observation.actual.expect("actual address")
    }

    fn http_client(address: TcpSocketAddress, target: &'static str) -> thread::JoinHandle<String> {
        thread::spawn(move || {
            let socket: SocketAddr = format!("{}:{}", address.host, address.port)
                .parse()
                .expect("socket addr");
            let mut stream = TcpStream::connect(socket).expect("client connect");
            stream
                .write_all(
                    format!("GET {target} HTTP/1.1\r\nHost: backend.test\r\n\r\n").as_bytes(),
                )
                .expect("write request");
            stream.shutdown(Shutdown::Write).expect("shutdown write");
            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read response");
            String::from_utf8(response).expect("utf8 response")
        })
    }

    fn config(path: PathBuf) -> LocalBackendCompositionConfig {
        LocalBackendCompositionConfig::local("api", "primary", path, "application")
    }

    #[test]
    fn standalone_composition_declares_expected_semantic_graph() {
        let path = unique_db_path("inspect");
        let composition = build_local_backend_composition(
            "onoal.composition.test.local-backend.inspect",
            config(path.clone()),
        )
        .expect("composition");

        assert_eq!(composition.resources().count(), 3);
        assert_eq!(composition.components().count(), 2);
        assert!(composition
            .resources()
            .any(|resource| resource.resource_id().as_str()
                == "onoal.package.networking.tcp.byte-stream"
                && resource.name().as_str() == "api"
                && resource
                    .realization()
                    .adapter_definition_id()
                    .expect("tcp adapter")
                    .as_str()
                    == "onoal.package.networking.tcp.loopback"));
        assert!(composition
            .resources()
            .any(|resource| resource.resource_id().as_str()
                == "onoal.package.data.relational-database"
                && resource.name().as_str() == "primary"
                && resource
                    .realization()
                    .adapter_definition_id()
                    .expect("sqlite adapter")
                    .as_str()
                    == "onoal.package.data.sqlite.relational-database"));
        assert!(composition
            .resources()
            .any(|resource| resource.resource_id().as_str()
                == "onoal.package.observability.logging.sink"
                && resource.name().as_str() == "application"
                && resource
                    .realization()
                    .adapter_definition_id()
                    .expect("console adapter")
                    .as_str()
                    == "onoal.package.observability.logging.console"));
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.networking.tcp.inspector"));
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.networking.http.server"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "transport"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "api"
                )));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn custom_names_are_preserved() {
        let path = unique_db_path("names");
        let composition = build_local_backend_composition(
            "onoal.composition.test.local-backend.names",
            LocalBackendCompositionConfig::local("admin", "state", path.clone(), "backend-log"),
        )
        .expect("composition");

        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "admin"));
        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "state"));
        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "backend-log"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn local_backend_runs_real_http_exchange() {
        let path = unique_db_path("http");
        let composition = build_local_backend_composition(
            "onoal.composition.test.local-backend.http",
            config(path.clone()),
        )
        .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.local-backend.http.instance",
        );
        let inspector = activate::<TcpTransportInspector>(&instance);
        let server = activate::<HttpServer>(&instance);
        let address = actual_address(&inspector);
        let client = http_client(address, "/backend");

        let request =
            block_on(server.serve_once(HttpResponse::new(200, b"local-backend".to_vec())))
                .expect("serve")
                .expect("request");
        let response = client.join().expect("client");

        assert_eq!(request.target, "/backend");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("\r\n\r\nlocal-backend"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn consumer_owned_app_uses_database_and_logging() {
        let path = unique_db_path("app");
        let composition = Fabric::new("onoal.composition.test.local-backend.app")
            .expect("fabric")
            .with(local_backend_stack(config(path.clone())))
            .with(test_app("primary", "application"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.local-backend.app.instance",
        );
        let app = activate::<TestLocalBackendApp>(&instance);

        let result = block_on(app.write_read_log("stored-value".to_owned()))
            .expect("app")
            .expect("result");
        assert_eq!(result.value.as_deref(), Some("stored-value"));
        assert_eq!(result.logged.target.as_deref(), Some("local-backend-test"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn database_persistence_survives_fresh_generation() {
        let path = unique_db_path("persist");
        let composition = Fabric::new("onoal.composition.test.local-backend.persist")
            .expect("fabric")
            .with(local_backend_stack(config(path.clone())))
            .with(test_app("primary", "application"))
            .build()
            .expect("composition");
        {
            let mut instance = started_instance(
                &composition,
                "onoal.composition.test.local-backend.persist.a",
            );
            let app = activate::<TestLocalBackendApp>(&instance);
            let result = block_on(app.write_read_log("durable-value".to_owned()))
                .expect("app")
                .expect("result");
            assert_eq!(result.value.as_deref(), Some("durable-value"));
            instance.stop().expect("stop");
        }
        {
            let mut instance = started_instance(
                &composition,
                "onoal.composition.test.local-backend.persist.b",
            );
            let app = activate::<TestLocalBackendApp>(&instance);
            let value = block_on(app.read_value())
                .expect("read")
                .expect("read result");
            assert_eq!(value.as_deref(), Some("durable-value"));
            instance.stop().expect("stop");
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn application_data_can_be_used_as_http_response_body_without_direct_component_relation() {
        let path = unique_db_path("http-app");
        let composition = Fabric::new("onoal.composition.test.local-backend.http-app")
            .expect("fabric")
            .with(local_backend_stack(config(path.clone())))
            .with(test_app("primary", "application"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.local-backend.http-app.instance",
        );
        let inspector = activate::<TcpTransportInspector>(&instance);
        let server = activate::<HttpServer>(&instance);
        let app = activate::<TestLocalBackendApp>(&instance);
        let app_result = block_on(app.write_read_log("response-from-db".to_owned()))
            .expect("app")
            .expect("result");
        let body = app_result.value.expect("value").into_bytes();
        let address = actual_address(&inspector);
        let client = http_client(address, "/from-app");

        let request = block_on(server.serve_once(HttpResponse::new(200, body)))
            .expect("serve")
            .expect("request");
        let response = client.join().expect("client");

        assert_eq!(request.target, "/from-app");
        assert!(response.ends_with("\r\n\r\nresponse-from-db"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn multiple_local_backend_stacks_hit_current_component_identity_law() {
        let first = unique_db_path("multi-a");
        let second = unique_db_path("multi-b");
        let result = Fabric::new("onoal.composition.test.local-backend.multi")
            .expect("fabric")
            .with(local_backend_stack(LocalBackendCompositionConfig::local(
                "api",
                "primary",
                first.clone(),
                "application",
            )))
            .with(local_backend_stack(LocalBackendCompositionConfig::local(
                "admin",
                "state",
                second.clone(),
                "backend-log",
            )))
            .build();

        assert!(
            result.is_err(),
            "Fabric v1 Component definition identity prevents two HttpServer/TcpTransportInspector occurrences"
        );
        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
    }
}
