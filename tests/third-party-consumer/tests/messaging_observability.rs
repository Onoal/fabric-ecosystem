mod support;

use fabric::prelude::*;
use fabric_package_messaging_queue::{FifoQueue, QueueSendResult};
use fabric_package_observability_counter::{CounterError, CounterIncrementResult, CounterMetric};
use fabric_package_observability_logging::{LogError, LogRecord, LogSink};
use futures::executor::block_on;
use support::fabric::{activate, started_instance};

#[derive(Clone, Debug, PartialEq, Eq)]
struct InstrumentedJobResult {
    first_send: QueueSendResult,
    second_send: QueueSendResult,
    processed: Option<Vec<u8>>,
    successes: u64,
    failures: u64,
    logged: LogRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InstrumentedJobError {
    Counter(CounterError),
    Log(LogError),
}

impl From<CounterError> for InstrumentedJobError {
    fn from(error: CounterError) -> Self {
        Self::Counter(error)
    }
}

impl From<LogError> for InstrumentedJobError {
    fn from(error: LogError) -> Self {
        Self::Log(error)
    }
}

fabric::component! {
    InstrumentedQueueConsumer {
        id: "onoal.test.third-party.messaging.instrumented-queue";

        relations {
            requires {
                jobs: FifoQueue;
                successful_jobs: CounterMetric;
                failed_jobs: CounterMetric;
                log: LogSink;
            }
        }

        api {
            fn run_jobs(&self) -> Result<InstrumentedJobResult, InstrumentedJobError>;
        }

        runtime {
            fn run_jobs(&self) -> Result<InstrumentedJobResult, InstrumentedJobError> {
                let first_send = self.relations().jobs.send(b"job".to_vec());
                match first_send {
                    QueueSendResult::Accepted => {
                        let _: CounterIncrementResult =
                            self.relations().successful_jobs.increment(1)?;
                    }
                    QueueSendResult::Full { .. } => {
                        let _: CounterIncrementResult =
                            self.relations().failed_jobs.increment(1)?;
                    }
                }

                let second_send = self.relations().jobs.send(b"overflow".to_vec());
                match second_send {
                    QueueSendResult::Accepted => {
                        let _: CounterIncrementResult =
                            self.relations().successful_jobs.increment(1)?;
                    }
                    QueueSendResult::Full { .. } => {
                        let _: CounterIncrementResult =
                            self.relations().failed_jobs.increment(1)?;
                    }
                }

                let processed = self.relations()
                    .jobs
                    .try_receive()
                    .map(|message| {
                        let mut output = b"third-party:".to_vec();
                        output.extend(message.payload);
                        output
                    });
                let logged = LogRecord::targeted(
                    fabric_package_observability_logging::LogLevel::Info,
                    "third-party-jobs",
                    "processed public queue job",
                );
                self.relations().log.emit(logged.clone())?;
                let successes = self.relations().successful_jobs.current()?;
                let failures = self.relations().failed_jobs.current()?;
                Ok(InstrumentedJobResult {
                    first_send,
                    second_send,
                    processed,
                    successes,
                    failures,
                    logged,
                })
            }
        }
    }
}

fn instrumented_queue_consumer(
    queue_name: &'static str,
    success_counter: &'static str,
    failure_counter: &'static str,
    log_name: &'static str,
) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("queue selection");
    let successes = CounterMetric::select(success_counter).expect("success counter selection");
    let failures = CounterMetric::select(failure_counter).expect("failure counter selection");
    let log = LogSink::select(log_name).expect("log selection");
    let component = InstrumentedQueueConsumer::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("jobs").expect("role"),
                fabric::authoring::Requires::<FifoQueue>::provisional(),
            ),
            &queue,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("successful_jobs").expect("role"),
                fabric::authoring::Requires::<CounterMetric>::provisional(),
            ),
            &successes,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("failed_jobs").expect("role"),
                fabric::authoring::Requires::<CounterMetric>::provisional(),
            ),
            &failures,
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
fn external_consumer_owns_queue_instrumentation_and_logging_behavior() {
    let composition = Fabric::new("onoal.test.third-party.messaging-observability")
        .expect("fabric")
        .with(fabric_package_messaging_queue::memory_queue("jobs", 1).expect("queue config"))
        .with(fabric_package_observability_counter::in_memory_counter(
            "successful-jobs",
        ))
        .with(fabric_package_observability_counter::in_memory_counter(
            "failed-jobs",
        ))
        .with(fabric_package_observability_logging::console_logging(
            "application-log",
        ))
        .with(instrumented_queue_consumer(
            "jobs",
            "successful-jobs",
            "failed-jobs",
            "application-log",
        ))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.messaging-observability.instance",
    );
    let consumer = activate::<InstrumentedQueueConsumer>(&instance);

    let result = block_on(consumer.run_jobs())
        .expect("exercise")
        .expect("job result");

    assert_eq!(result.first_send, QueueSendResult::Accepted);
    assert_eq!(result.second_send, QueueSendResult::Full { capacity: 1 });
    assert_eq!(result.processed, Some(b"third-party:job".to_vec()));
    assert_eq!(result.successes, 1);
    assert_eq!(result.failures, 1);
    assert_eq!(result.logged.target.as_deref(), Some("third-party-jobs"));
}
