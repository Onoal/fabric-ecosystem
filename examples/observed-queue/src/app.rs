use fabric::prelude::*;
use fabric_package_messaging_queue::{FifoQueue, QueueSendResult};
use fabric_package_observability_counter::{CounterError, CounterMetric};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ObservedQueueCounts {
    pub(crate) successes: u64,
    pub(crate) failures: u64,
}

fabric::component! {
    pub ObservedQueueProducer {
        id: "fabric.ecosystem.example.observed-queue.producer";

        relations {
            requires {
                queue: FifoQueue;
                successes: CounterMetric;
                failures: CounterMetric;
            }
        }

        api {
            fn send_observed(&self, payload: Vec<u8>) -> Result<QueueSendResult, CounterError>;
            fn observed_counts(&self) -> Result<ObservedQueueCounts, CounterError>;
        }

        runtime {
            fn send_observed(&self, payload: Vec<u8>) -> Result<QueueSendResult, CounterError> {
                let result = self.relations().queue.send(payload);
                match result {
                    QueueSendResult::Accepted => {
                        let _ = self.relations().successes.increment(1)?;
                    }
                    QueueSendResult::Full { .. } => {
                        let _ = self.relations().failures.increment(1)?;
                    }
                }
                Ok(result)
            }

            fn observed_counts(&self) -> Result<ObservedQueueCounts, CounterError> {
                Ok(ObservedQueueCounts {
                    successes: self.relations().successes.current()?,
                    failures: self.relations().failures.current()?,
                })
            }
        }
    }
}

pub(crate) fn observed_queue_producer() -> impl IntoFabricContribution {
    let queue = FifoQueue::select("events").expect("valid queue resource name");
    let successes = CounterMetric::select("successful-sends").expect("valid success counter name");
    let failures = CounterMetric::select("failed-sends").expect("valid failure counter name");
    let component = ObservedQueueProducer::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("queue").expect("role"),
                fabric::authoring::Requires::<FifoQueue>::provisional(),
            ),
            &queue,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("successes").expect("role"),
                fabric::authoring::Requires::<CounterMetric>::provisional(),
            ),
            &successes,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("failures").expect("role"),
                fabric::authoring::Requires::<CounterMetric>::provisional(),
            ),
            &failures,
        );
    FabricContribution::new().component(component)
}
