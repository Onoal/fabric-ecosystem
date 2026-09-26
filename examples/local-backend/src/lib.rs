//! Local backend Composition example for Fabric Ecosystem.

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use fabric::prelude::*;
use fabric_composition_local_backend::{local_backend_stack, LocalBackendCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{
    TcpSocketAddress, TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
use fabric_package_observability_logging::{LogRecord, LogSink};
use fabric_package_relational_database::{RelationalDatabase, RelationalValue};
use futures::executor::block_on;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExampleBackendResult {
    pub stored: String,
    pub logged: LogRecord,
    pub http_response: String,
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
            fn write_read_log(&self, value: String) -> Result<ExampleBackendResult, String>;
        }

        runtime {
            fn write_read_log(&self, value: String) -> Result<ExampleBackendResult, String> {
                self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS example_items (id INTEGER PRIMARY KEY, value TEXT NOT NULL)".to_owned(),
                    vec![],
                ).map_err(|error| error.to_string())?;
                self.relations().database.execute(
                    "INSERT INTO example_items (id, value) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET value = excluded.value".to_owned(),
                    vec![RelationalValue::Integer(1), RelationalValue::Text(value)],
                ).map_err(|error| error.to_string())?;
                let rows = self.relations().database.query(
                    "SELECT value FROM example_items WHERE id = ?1".to_owned(),
                    vec![RelationalValue::Integer(1)],
                ).map_err(|error| error.to_string())?;
                let stored = rows.rows().first()
                    .and_then(|row| row.get(0))
                    .and_then(|value| match value {
                        RelationalValue::Text(value) => Some(value.clone()),
                        _ => None,
                    })
                    .ok_or_else(|| "example row missing".to_owned())?;
                let logged = LogRecord::targeted(
                    fabric_package_observability_logging::LogLevel::Info,
                    "local-backend-example",
                    "example stored local data",
                );
                self.relations().log.emit(logged.clone()).map_err(|error| error.to_string())?;
                Ok(ExampleBackendResult {
                    stored,
                    logged,
                    http_response: String::new(),
                })
            }
        }
    }
}

pub fn run() -> Result<ExampleBackendResult, Box<dyn std::error::Error>> {
    let path = unique_db_path();
    let database = RelationalDatabase::select("primary").expect("database selection");
    let log = LogSink::select("application").expect("log selection");
    let app = ExampleBackendApp::define()
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
    let composition = Fabric::new("fabric.ecosystem.example.local-backend")?
        .with(local_backend_stack(LocalBackendCompositionConfig::local(
            "api",
            "primary",
            path.clone(),
            "application",
        )))
        .component(app)
        .build()?;
    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.local-backend.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let app = instance.component::<ExampleBackendApp>()?;
    app.reconcile()?;
    let mut result = block_on(app.write_read_log("local-data".to_owned()))??;
    let inspector = instance.component::<TcpTransportInspector>()?;
    inspector.reconcile()?;
    let address = block_on(inspector.inspect_transport())?
        .actual
        .ok_or("local backend did not bind a TCP address")?;
    let server = instance.component::<HttpServer>()?;
    server.reconcile()?;
    let client = http_client(address);
    let exchange = block_on(server.accept_exchange())??;
    let request_target = exchange.request().target.clone();
    result.stored = format!("{} via {}", result.stored, request_target);
    exchange.respond(HttpResponse::new(200, result.stored.clone().into_bytes()))?;
    result.http_response = client.join().expect("client");

    instance.stop()?;
    let _ = std::fs::remove_file(path);
    Ok(result)
}

fn unique_db_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "onoal-fabric-local-backend-example-{}-{nanos}.db",
        std::process::id()
    ))
}

fn http_client(address: TcpSocketAddress) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let socket: SocketAddr = format!("{}:{}", address.host, address.port)
            .parse()
            .expect("socket addr");
        let mut stream = TcpStream::connect(socket).expect("client connect");
        stream
            .write_all(b"GET /local-backend HTTP/1.1\r\nHost: example.local\r\n\r\n")
            .expect("write request");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        String::from_utf8(response).expect("utf8 response")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_extends_local_backend_foundation_with_application_behavior() {
        let result = run().expect("example");
        assert_eq!(result.stored, "local-data via /local-backend");
        assert_eq!(
            result.logged.target.as_deref(),
            Some("local-backend-example")
        );
        assert!(result
            .http_response
            .ends_with("\r\n\r\nlocal-data via /local-backend"));
    }
}
