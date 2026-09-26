//! Observed queue example for Fabric Ecosystem.

use fabric::prelude::*;
use fabric_package_messaging_queue::{FifoQueue, QueueSendResult};
use fabric_package_observability_counter::{in_memory_counter, CounterMetric, DualCounterSnapshot};

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
            fn send_observed(&self, payload: Vec<u8>) -> QueueSendResult;
            fn observed_counts(&self) -> DualCounterSnapshot;
        }

        runtime {
            fn send_observed(&self, payload: Vec<u8>) -> QueueSendResult {
                let result = self.relations().queue.send(payload);
                match result {
                    QueueSendResult::Accepted => {
                        let _ = self.relations().successes.increment(1);
                    }
                    QueueSendResult::Full { .. } => {
                        let _ = self.relations().failures.increment(1);
                    }
                }
                result
            }

            fn observed_counts(&self) -> DualCounterSnapshot {
                DualCounterSnapshot {
                    successes: self.relations().successes.current(),
                    failures: self.relations().failures.current(),
                }
            }
        }
    }
}

pub fn observed_queue_producer() -> impl IntoFabricContribution {
    let queue = FifoQueue::select("events").expect("queue selection");
    let successes = CounterMetric::select("successful-sends").expect("success counter");
    let failures = CounterMetric::select("failed-sends").expect("failure counter");
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

pub fn run() -> Result<DualCounterSnapshot, Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.observed-queue")?
        .with(fabric_package_messaging_queue::memory_queue("events", 1)?)
        .with(in_memory_counter("successful-sends"))
        .with(in_memory_counter("failed-sends"))
        .with(observed_queue_producer())
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.observed-queue.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let producer = instance.component::<ObservedQueueProducer>()?;
    producer.reconcile()?;
    assert_eq!(
        futures::executor::block_on(producer.send_observed(b"first".to_vec()))?,
        QueueSendResult::Accepted
    );
    assert_eq!(
        futures::executor::block_on(producer.send_observed(b"second".to_vec()))?,
        QueueSendResult::Full { capacity: 1 }
    );
    let counts = futures::executor::block_on(producer.observed_counts())?;

    instance.stop()?;
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_builds_materializes_records_metrics_and_stops() {
        assert_eq!(
            run().expect("observed queue example"),
            DualCounterSnapshot {
                successes: 1,
                failures: 1
            }
        );
    }
}
