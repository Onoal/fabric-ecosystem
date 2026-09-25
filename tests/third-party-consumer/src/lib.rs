//! Third-party-style consumer fixture for Fabric packages.

use fabric::prelude::*;
use fabric_package_key_value::KeyValue;
use fabric_package_messaging_queue::{FifoQueue, QueueMessage, QueueSendResult};
use fabric_package_networking_tcp::{
    TcpByteStreamTransport, TcpConnectResult, TcpProbeObservation, TcpTransportError,
};
use fabric_package_process_runtime::{ExecutionEnvironment, ProcessOutput, ProcessRuntime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumerOutput {
    pub stored: Option<Vec<u8>>,
    pub process: ProcessOutput,
    pub environment: ExecutionEnvironment,
    pub message_send: QueueSendResult,
    pub message: Option<QueueMessage>,
    pub transport_observation: TcpProbeObservation,
    pub transport_connect: Result<(), TcpTransportError>,
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
                ConsumerOutput {
                    stored,
                    process,
                    environment: self.relations().environment.describe(),
                    message_send,
                    message,
                    transport_observation,
                    transport_connect,
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
        );

    FabricContribution::new().component(component)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn third_party_consumer_composes_package_contributions_without_package_identity() {
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
            .with(application())
            .build()
            .expect("composition");

        assert_eq!(composition.resources().count(), 4);
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

        let mut instance = composition
            .materialize_on(
                "onoal.package.test.third-party.instance",
                &HostDescriptor::native(),
            )
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
    }
}
