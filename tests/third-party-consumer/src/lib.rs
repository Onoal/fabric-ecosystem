//! Third-party-style consumer fixture for Fabric packages.

use fabric::prelude::*;
use fabric_package_key_value::KeyValue;
use fabric_package_messaging_queue::{FifoQueue, QueueMessage, QueueSendResult};
use fabric_package_networking_tcp::{
    TcpByteStreamTransport, TcpConnectResult, TcpProbeObservation, TcpTransportError,
};
use fabric_package_observability_counter::{CounterMetric, DualCounterSnapshot};
use fabric_package_observability_logging::{LogRecord, LogSink};
use fabric_package_process_runtime::{ExecutionEnvironment, ProcessOutput, ProcessRuntime};
use fabric_package_relational_database::{
    RelationalDatabase, RelationalQueryResult, RelationalValue,
};
use fabric_package_security_ed25519::{verify_ed25519_signature, Ed25519Signer, SignedPayload};

#[derive(Clone, Debug, PartialEq)]
pub struct ConsumerOutput {
    pub stored: Option<Vec<u8>>,
    pub process: ProcessOutput,
    pub environment: ExecutionEnvironment,
    pub message_send: QueueSendResult,
    pub message: Option<QueueMessage>,
    pub transport_observation: TcpProbeObservation,
    pub transport_connect: Result<(), TcpTransportError>,
    pub signed: SignedPayload,
    pub signature_valid: bool,
    pub metric_counts: DualCounterSnapshot,
    pub database_rows: RelationalQueryResult,
    pub logged: LogRecord,
}

fabric::component! {
    pub PackageConsumer {
        id: "onoal.package.test.third-party.consumer";

        relations {
            requires {
                store: KeyValue;
                process: ProcessRuntime;
                queue: FifoQueue;
                transport: TcpByteStreamTransport;
                signer: Ed25519Signer;
                successful_sends: CounterMetric;
                failed_sends: CounterMetric;
                database: RelationalDatabase;
                log: LogSink;
                environment: fabric_package_process_runtime::LocalExecutionEnvironment;
            }
        }

        api {
            fn exercise(&self) -> ConsumerOutput;
        }

        runtime {
            fn exercise(&self) -> ConsumerOutput {
                self.relations()
                    .store
                    .set("third-party".to_owned(), b"package-value".to_vec());
                let stored = self.relations().store.get("third-party".to_owned());
                let process = self.relations().process.run(
                    "sh".to_owned(),
                    vec!["-c".to_owned(), "printf third-party".to_owned()],
                );
                let message_send = self
                    .relations()
                    .queue
                    .send(b"third-party-message".to_vec());
                match message_send {
                    QueueSendResult::Accepted => {
                        let _ = self.relations().successful_sends.increment(1);
                    }
                    QueueSendResult::Full { .. } => {
                        let _ = self.relations().failed_sends.increment(1);
                    }
                }
                let message = self.relations().queue.try_receive();
                let transport_observation = TcpProbeObservation {
                    requested: self.relations().transport.requested_bind_address(),
                    actual: self.relations().transport.actual_bound_address(),
                    accepted_connections: self.relations().transport.accepted_connections(),
                };
                let transport_connect = match transport_observation.actual.clone() {
                    Some(address) => match self.relations().transport.connect(address) {
                        TcpConnectResult::Connected(connection) => connection.shutdown_both(),
                        TcpConnectResult::Failed(error) => Err(error),
                    },
                    None => Err(TcpTransportError {
                        kind: fabric_package_networking_tcp::TcpTransportErrorKind::NotStarted,
                        detail: "tcp transport has no live bound address".to_owned(),
                    }),
                };
                let public_key = self.relations()
                    .signer
                    .public_key()
                    .expect("third-party signer public key");
                let signature = self.relations()
                    .signer
                    .sign(b"third-party-signed".to_vec())
                    .expect("third-party signer signs");
                let signed = SignedPayload {
                    payload: b"third-party-signed".to_vec(),
                    public_key,
                    signature,
                };
                let signature_valid = verify_ed25519_signature(
                    &signed.public_key,
                    &signed.payload,
                    &signed.signature,
                )
                .expect("verify third-party signature");
                let metric_counts = DualCounterSnapshot {
                    successes: self.relations().successful_sends.current(),
                    failures: self.relations().failed_sends.current(),
                };
                self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS third_party_items (id INTEGER, name TEXT NOT NULL)".to_owned(),
                    vec![],
                )
                .expect("third-party creates table");
                self.relations().database.execute(
                    "INSERT INTO third_party_items (id, name) VALUES (?1, ?2)".to_owned(),
                    vec![
                        RelationalValue::Integer(1),
                        RelationalValue::Text("public-consumer".to_owned()),
                    ],
                )
                .expect("third-party inserts row");
                let database_rows = self.relations().database.query(
                    "SELECT id, name FROM third_party_items ORDER BY id".to_owned(),
                    vec![],
                )
                .expect("third-party queries row");
                let logged = LogRecord::targeted(
                    fabric_package_observability_logging::LogLevel::Info,
                    "third-party",
                    "public consumer exercised packages",
                );
                self.relations()
                    .log
                    .emit(logged.clone())
                    .expect("third-party emits log record");
                ConsumerOutput {
                    stored,
                    process,
                    environment: self.relations().environment.describe(),
                    message_send,
                    message,
                    transport_observation,
                    transport_connect,
                    signed,
                    signature_valid,
                    metric_counts,
                    database_rows,
                    logged,
                }
            }
        }
    }
}

pub fn application() -> impl IntoFabricContribution {
    let store = KeyValue::select("primary").expect("store selection");
    let process = ProcessRuntime::select("local").expect("process selection");
    let queue = FifoQueue::select("events").expect("queue selection");
    let transport = TcpByteStreamTransport::select("api").expect("transport selection");
    let signer = Ed25519Signer::select("release").expect("signer selection");
    let successful_sends =
        CounterMetric::select("successful-sends").expect("successful send counter");
    let failed_sends = CounterMetric::select("failed-sends").expect("failed send counter");
    let database = RelationalDatabase::select("primary-db").expect("database selection");
    let log = LogSink::select("application-log").expect("log sink selection");
    let component = PackageConsumer::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("store").expect("role"),
                fabric::authoring::Requires::<KeyValue>::provisional(),
            ),
            &store,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("process").expect("role"),
                fabric::authoring::Requires::<ProcessRuntime>::provisional(),
            ),
            &process,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("queue").expect("role"),
                fabric::authoring::Requires::<FifoQueue>::provisional(),
            ),
            &queue,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("transport").expect("role"),
                fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
            ),
            &transport,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("signer").expect("role"),
                fabric::authoring::Requires::<Ed25519Signer>::provisional(),
            ),
            &signer,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("successful_sends").expect("role"),
                fabric::authoring::Requires::<CounterMetric>::provisional(),
            ),
            &successful_sends,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("failed_sends").expect("role"),
                fabric::authoring::Requires::<CounterMetric>::provisional(),
            ),
            &failed_sends,
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_composition_http_server::{
        build_http_server_composition, http_server_stack, HttpServerCompositionConfig,
    };
    use fabric_composition_local_backend::{
        build_local_backend_composition, local_backend_stack, LocalBackendCompositionConfig,
    };
    use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
    use fabric_package_networking_tcp::{TcpTransportProbe, TcpTransportProbeInstanceApi};
    use futures::executor::block_on;
    use std::io::{Read, Write};
    use std::net::{Shutdown, SocketAddr, TcpStream};
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone, Debug, PartialEq)]
    struct ThirdPartyBackendAppResult {
        rows: RelationalQueryResult,
        logged: LogRecord,
    }

    fabric::component! {
        ThirdPartyBackendApp {
            id: "onoal.package.test.third-party.local-backend.app";

            relations {
                requires {
                    database: RelationalDatabase;
                    log: LogSink;
                }
            }

            api {
                fn exercise_backend(&self) -> Result<ThirdPartyBackendAppResult, String>;
            }

            runtime {
                fn exercise_backend(&self) -> Result<ThirdPartyBackendAppResult, String> {
                    self.relations().database.execute(
                        "CREATE TABLE IF NOT EXISTS third_party_backend_items (id INTEGER, value TEXT NOT NULL)".to_owned(),
                        vec![],
                    ).map_err(|error| error.to_string())?;
                    self.relations().database.execute(
                        "INSERT INTO third_party_backend_items (id, value) VALUES (?1, ?2)".to_owned(),
                        vec![
                            RelationalValue::Integer(1),
                            RelationalValue::Text("backend-consumer".to_owned()),
                        ],
                    ).map_err(|error| error.to_string())?;
                    let rows = self.relations().database.query(
                        "SELECT id, value FROM third_party_backend_items ORDER BY id".to_owned(),
                        vec![],
                    ).map_err(|error| error.to_string())?;
                    let logged = LogRecord::targeted(
                        fabric_package_observability_logging::LogLevel::Info,
                        "third-party-local-backend",
                        "third-party backend app used local backend foundation",
                    );
                    self.relations().log.emit(logged.clone()).map_err(|error| error.to_string())?;
                    Ok(ThirdPartyBackendAppResult { rows, logged })
                }
            }
        }
    }

    fn third_party_backend_app(
        database_name: &'static str,
        log_name: &'static str,
    ) -> impl IntoFabricContribution {
        let database = RelationalDatabase::select(database_name).expect("database selection");
        let log = LogSink::select(log_name).expect("log selection");
        let component = ThirdPartyBackendApp::define()
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

    fn unique_sqlite_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "onoal-fabric-third-party-sqlite-{}-{nanos}.db",
            std::process::id()
        ))
    }

    #[test]
    fn third_party_consumer_composes_package_contributions_without_package_identity() {
        let database_path = unique_sqlite_path();
        #[cfg(target_os = "linux")]
        let host = {
            let detected = fabric_host_linux::detect_linux_host().expect("linux host detection");
            assert_eq!(
                detected.operating_system(),
                &fabric_host_linux::linux_operating_system()
            );
            detected
        };
        #[cfg(not(target_os = "linux"))]
        let host = HostDescriptor::native();

        let composition = Fabric::new("onoal.package.test.third-party")
            .expect("fabric")
            .with(fabric_package_key_value::audited_memory_key_value(
                "primary",
            ))
            .with(fabric_package_process_runtime::local_execution_environment())
            .with(fabric_package_process_runtime::local_process_runtime(
                "local",
            ))
            .with(fabric_package_messaging_queue::memory_queue("events", 4))
            .with(fabric_package_networking_tcp::loopback_tcp_transport("api"))
            .with(fabric_package_security_ed25519::ephemeral_ed25519_signer(
                "release",
            ))
            .with(fabric_package_observability_counter::in_memory_counter(
                "successful-sends",
            ))
            .with(fabric_package_observability_counter::in_memory_counter(
                "failed-sends",
            ))
            .with(fabric_package_sqlite::sqlite_database(
                "primary-db",
                database_path.clone(),
            ))
            .with(fabric_package_observability_logging::console_logging(
                "application-log",
            ))
            .with(application())
            .build()
            .expect("composition");

        assert_eq!(composition.resources().count(), 9);
        assert_eq!(composition.systems().count(), 1);
        assert_eq!(composition.components().count(), 1);
        assert_eq!(
            composition
                .resources()
                .find(|resource| resource.name().as_str() == "primary")
                .expect("key-value")
                .augmentations()
                .count(),
            1
        );
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "store"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "process"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "queue"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "transport"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "signer"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "successful_sends"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "failed_sends"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "database"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "log"));

        let mut instance = composition
            .materialize_on("onoal.package.test.third-party.instance", &host)
            .expect("instance");
        instance.start().expect("start");

        let app = instance.component::<PackageConsumer>().expect("consumer");
        app.reconcile().expect("component reconcile");
        let output = block_on(app.exercise()).expect("exercise");

        assert_eq!(output.stored, Some(b"package-value".to_vec()));
        assert!(output.process.success());
        assert_eq!(output.process.stdout_utf8(), "third-party");
        assert!(!output.environment.os.is_empty());
        assert_eq!(output.message_send, QueueSendResult::Accepted);
        assert_eq!(
            output.message,
            Some(QueueMessage {
                payload: b"third-party-message".to_vec()
            })
        );
        assert!(output.transport_observation.actual.is_some());
        assert!(output.transport_connect.is_ok());
        assert_eq!(output.signed.payload, b"third-party-signed".to_vec());
        assert!(output.signature_valid);
        assert!(verify_ed25519_signature(
            &output.signed.public_key,
            &output.signed.payload,
            &output.signed.signature,
        )
        .expect("public verification"));
        assert_eq!(
            output.metric_counts,
            DualCounterSnapshot {
                successes: 1,
                failures: 0
            }
        );
        assert_eq!(
            output.database_rows.rows()[0].values(),
            &[
                RelationalValue::Integer(1),
                RelationalValue::Text("public-consumer".to_owned()),
            ]
        );
        assert_eq!(output.logged.target.as_deref(), Some("third-party"));
        let _ = std::fs::remove_file(database_path);
    }

    #[test]
    fn third_party_consumer_uses_reusable_http_server_composition() {
        #[cfg(target_os = "linux")]
        let host = fabric_host_linux::detect_linux_host().expect("linux host detection");
        #[cfg(not(target_os = "linux"))]
        let host = HostDescriptor::native();

        let composition = build_http_server_composition(
            "onoal.package.test.third-party.http",
            HttpServerCompositionConfig::local("api"),
        )
        .expect("composition");
        let mut instance = composition
            .materialize_on("onoal.package.test.third-party.http.instance", &host)
            .expect("instance");
        instance.start().expect("start");

        let probe = instance
            .component::<TcpTransportProbe>()
            .expect("transport probe");
        probe.reconcile().expect("probe reconcile");
        let server = instance.component::<HttpServer>().expect("http server");
        server.reconcile().expect("server reconcile");
        let address = block_on(probe.observe_transport())
            .expect("observe transport")
            .actual
            .expect("bound TCP address");
        let client = thread::spawn(move || {
            let socket: SocketAddr = format!("{}:{}", address.host, address.port)
                .parse()
                .expect("socket addr");
            let mut stream = TcpStream::connect(socket).expect("client connect");
            stream
                .write_all(b"GET /third-party HTTP/1.1\r\nHost: consumer.test\r\n\r\n")
                .expect("write request");
            stream.shutdown(Shutdown::Write).expect("shutdown write");
            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read response");
            response
        });

        let request =
            block_on(server.serve_once(HttpResponse::new(200, b"third-party-http".to_vec())))
                .expect("serve")
                .expect("request");
        let response = String::from_utf8(client.join().expect("client")).expect("response utf8");

        assert_eq!(request.method, "GET");
        assert_eq!(request.target, "/third-party");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("\r\n\r\nthird-party-http"));
    }

    #[test]
    fn third_party_consumer_reuses_http_stack_inside_larger_fabric_build() {
        let composition = Fabric::new("onoal.package.test.third-party.http.reuse")
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
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.networking.http.server"));
    }

    #[test]
    fn third_party_consumer_uses_standalone_local_backend_composition() {
        #[cfg(target_os = "linux")]
        let host = fabric_host_linux::detect_linux_host().expect("linux host detection");
        #[cfg(not(target_os = "linux"))]
        let host = HostDescriptor::native();
        let database_path = unique_sqlite_path();

        let composition = build_local_backend_composition(
            "onoal.package.test.third-party.local-backend",
            LocalBackendCompositionConfig::local(
                "api",
                "primary-db",
                database_path.clone(),
                "application-log",
            ),
        )
        .expect("local backend composition");
        let mut instance = composition
            .materialize_on(
                "onoal.package.test.third-party.local-backend.instance",
                &host,
            )
            .expect("instance");
        instance.start().expect("start");

        let probe = instance
            .component::<TcpTransportProbe>()
            .expect("transport probe");
        probe.reconcile().expect("probe reconcile");
        let server = instance.component::<HttpServer>().expect("http server");
        server.reconcile().expect("server reconcile");
        let address = block_on(probe.observe_transport())
            .expect("observe transport")
            .actual
            .expect("bound TCP address");
        let client = thread::spawn(move || {
            let socket: SocketAddr = format!("{}:{}", address.host, address.port)
                .parse()
                .expect("socket addr");
            let mut stream = TcpStream::connect(socket).expect("client connect");
            stream
                .write_all(b"GET /local-backend HTTP/1.1\r\nHost: consumer.test\r\n\r\n")
                .expect("write request");
            stream.shutdown(Shutdown::Write).expect("shutdown write");
            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read response");
            response
        });

        let request = block_on(server.serve_once(HttpResponse::new(
            200,
            b"third-party-local-backend".to_vec(),
        )))
        .expect("serve")
        .expect("request");
        let response = String::from_utf8(client.join().expect("client")).expect("response utf8");

        assert_eq!(request.target, "/local-backend");
        assert!(response.ends_with("\r\n\r\nthird-party-local-backend"));
        let _ = std::fs::remove_file(database_path);
    }

    #[test]
    fn third_party_application_extends_local_backend_stack() {
        let database_path = unique_sqlite_path();
        let composition = Fabric::new("onoal.package.test.third-party.local-backend.extension")
            .expect("fabric")
            .with(local_backend_stack(LocalBackendCompositionConfig::local(
                "api",
                "primary-db",
                database_path.clone(),
                "application-log",
            )))
            .with(third_party_backend_app("primary-db", "application-log"))
            .build()
            .expect("composition");
        let mut instance = composition
            .materialize_on(
                "onoal.package.test.third-party.local-backend.extension.instance",
                &HostDescriptor::native(),
            )
            .expect("instance");
        instance.start().expect("start");

        let app = instance
            .component::<ThirdPartyBackendApp>()
            .expect("consumer");
        app.reconcile().expect("component reconcile");
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
        instance.stop().expect("stop");
        let _ = std::fs::remove_file(database_path);
    }
}
