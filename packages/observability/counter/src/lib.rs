//! Counter metric observability package for Fabric.
//!
//! A counter is aggregate telemetry: a monotonic value for one named Resource
//! occurrence. It is not Fabric observation, health, logging, tracing, audit,
//! history, identity, or durable evidence.

use std::sync::atomic::{AtomicU64, Ordering};

use fabric::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CounterIncrementResult {
    Updated { value: u64 },
    Overflow { current: u64, attempted: u64 },
}

impl CounterIncrementResult {
    pub fn value(&self) -> Option<u64> {
        match self {
            Self::Updated { value } => Some(*value),
            Self::Overflow { .. } => None,
        }
    }
}

#[derive(Default)]
pub struct InMemoryCounterState {
    value: AtomicU64,
}

impl InMemoryCounterState {
    fn increment(&self, amount: u64) -> CounterIncrementResult {
        let mut current = self.value.load(Ordering::SeqCst);
        loop {
            let Some(next) = current.checked_add(amount) else {
                return CounterIncrementResult::Overflow {
                    current,
                    attempted: amount,
                };
            };
            match self
                .value
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return CounterIncrementResult::Updated { value: next },
                Err(observed) => current = observed,
            }
        }
    }
}

fabric::resource! {
    pub CounterMetric {
        id: "onoal.package.observability.counter.metric";

        api {
            fn current(&self) -> u64;
            fn increment(&self, amount: u64) -> CounterIncrementResult;
        }
    }
}

fabric::adapter! {
    pub InMemoryCounter for CounterMetric {
        id: "onoal.package.observability.counter.in-memory";

        state {
            InMemoryCounterState = InMemoryCounterState::default();
        }

        runtime {
            fn current(&self) -> u64 {
                self.state.get().value.load(Ordering::SeqCst)
            }

            fn increment(&self, amount: u64) -> CounterIncrementResult {
                self.state.get().increment(amount)
            }
        }
    }
}

fabric::component! {
    pub CounterIncrementer {
        id: "onoal.package.observability.counter.incrementer";

        relations {
            requires {
                counter: CounterMetric;
            }
        }

        api {
            fn increment_by(&self, amount: u64) -> CounterIncrementResult;
            fn current(&self) -> u64;
        }

        runtime {
            fn increment_by(&self, amount: u64) -> CounterIncrementResult {
                self.relations().counter.increment(amount)
            }

            fn current(&self) -> u64 {
                self.relations().counter.current()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterBatchResult {
    pub updates: Vec<CounterIncrementResult>,
    pub final_value: u64,
}

fabric::component! {
    pub CounterBatchIncrementer {
        id: "onoal.package.observability.counter.batch-incrementer";

        relations {
            requires {
                counter: CounterMetric;
            }
        }

        api {
            fn increment_many(&self, times: u64, amount: u64) -> CounterBatchResult;
        }

        runtime {
            fn increment_many(&self, times: u64, amount: u64) -> CounterBatchResult {
                let mut updates = Vec::new();
                for _ in 0..times {
                    let update = self.relations().counter.increment(amount);
                    let overflowed = matches!(update, CounterIncrementResult::Overflow { .. });
                    updates.push(update);
                    if overflowed {
                        break;
                    }
                }
                CounterBatchResult {
                    updates,
                    final_value: self.relations().counter.current(),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualCounterSnapshot {
    pub successes: u64,
    pub failures: u64,
}

fabric::component! {
    pub DualCounterRecorder {
        id: "onoal.package.observability.counter.dual-recorder";

        relations {
            requires {
                successes: CounterMetric;
                failures: CounterMetric;
            }
        }

        api {
            fn record_success(&self) -> CounterIncrementResult;
            fn record_failure(&self) -> CounterIncrementResult;
            fn snapshot(&self) -> DualCounterSnapshot;
        }

        runtime {
            fn record_success(&self) -> CounterIncrementResult {
                self.relations().successes.increment(1)
            }

            fn record_failure(&self) -> CounterIncrementResult {
                self.relations().failures.increment(1)
            }

            fn snapshot(&self) -> DualCounterSnapshot {
                DualCounterSnapshot {
                    successes: self.relations().successes.current(),
                    failures: self.relations().failures.current(),
                }
            }
        }
    }
}

pub fn in_memory_counter(name: &'static str) -> impl IntoFabricContribution {
    let selected = CounterMetric::select(name).expect("valid CounterMetric resource name");
    FabricContribution::new().resource(
        selected
            .using(InMemoryCounter::new())
            .expect("InMemoryCounter supports CounterMetric"),
    )
}

pub fn counter_incrementer(counter_name: &'static str) -> impl IntoFabricContribution {
    let counter = CounterMetric::select(counter_name).expect("valid CounterMetric resource name");
    let component = CounterIncrementer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("counter").expect("role"),
            fabric::authoring::Requires::<CounterMetric>::provisional(),
        ),
        &counter,
    );
    FabricContribution::new().component(component)
}

pub fn counter_batch_incrementer(counter_name: &'static str) -> impl IntoFabricContribution {
    let counter = CounterMetric::select(counter_name).expect("valid CounterMetric resource name");
    let component = CounterBatchIncrementer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("counter").expect("role"),
            fabric::authoring::Requires::<CounterMetric>::provisional(),
        ),
        &counter,
    );
    FabricContribution::new().component(component)
}

pub fn dual_counter_recorder(
    successes_name: &'static str,
    failures_name: &'static str,
) -> impl IntoFabricContribution {
    let successes =
        CounterMetric::select(successes_name).expect("valid successes CounterMetric name");
    let failures = CounterMetric::select(failures_name).expect("valid failures CounterMetric name");
    let component = DualCounterRecorder::define()
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

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_package_messaging_queue::{FifoQueue, QueueSendResult};
    use futures::executor::block_on;

    fabric::component! {
        ObservedQueueProducer {
            id: "onoal.package.observability.counter.test-observed-queue-producer";

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

    fn observed_queue_component() -> impl IntoFabricContribution {
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

    #[test]
    fn composition_declares_counter_and_realization_without_metric_values() {
        let helper = Fabric::new("onoal.package.test.observability.inspect.helper")
            .expect("fabric")
            .with(in_memory_counter("requests"))
            .build()
            .expect("composition");
        let explicit = Fabric::new("onoal.package.test.observability.inspect.explicit")
            .expect("fabric")
            .resource(
                CounterMetric::select("requests")
                    .expect("counter")
                    .using(InMemoryCounter::new())
                    .expect("adapter"),
            )
            .build()
            .expect("composition");

        let helper_counter = helper.resources().next().expect("helper counter");
        let explicit_counter = explicit.resources().next().expect("explicit counter");
        assert_eq!(helper_counter.resource_id(), explicit_counter.resource_id());
        assert_eq!(helper_counter.name(), explicit_counter.name());
        assert_eq!(
            helper_counter
                .realization()
                .adapter_definition_id()
                .expect("helper adapter"),
            explicit_counter
                .realization()
                .adapter_definition_id()
                .expect("explicit adapter")
        );
        assert_eq!(helper.components().count(), 0);
        assert_eq!(helper.resources().count(), 1);
    }

    #[test]
    fn counter_is_monotonic_and_overflow_is_explicit() {
        let composition = Fabric::new("onoal.package.test.observability.counter")
            .expect("fabric")
            .with(in_memory_counter("requests"))
            .with(counter_incrementer("requests"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.package.test.observability.counter.instance",
        );
        let counter = activate::<CounterIncrementer>(&instance);

        assert_eq!(block_on(counter.current()).expect("current"), 0);
        assert_eq!(
            block_on(counter.increment_by(3)).expect("increment"),
            CounterIncrementResult::Updated { value: 3 }
        );
        assert_eq!(
            block_on(counter.increment_by(0)).expect("zero increment"),
            CounterIncrementResult::Updated { value: 3 }
        );
        assert_eq!(
            block_on(counter.increment_by(u64::MAX - 3)).expect("max increment"),
            CounterIncrementResult::Updated { value: u64::MAX }
        );
        assert_eq!(
            block_on(counter.increment_by(1)).expect("overflow"),
            CounterIncrementResult::Overflow {
                current: u64::MAX,
                attempted: 1
            }
        );
        assert_eq!(
            block_on(counter.current()).expect("after overflow"),
            u64::MAX
        );
    }

    #[test]
    fn multiple_counter_occurrences_are_independent_by_relation_role() {
        let composition = Fabric::new("onoal.package.test.observability.occurrences")
            .expect("fabric")
            .with(in_memory_counter("successful-operations"))
            .with(in_memory_counter("failed-operations"))
            .with(dual_counter_recorder(
                "successful-operations",
                "failed-operations",
            ))
            .build()
            .expect("composition");
        assert_eq!(composition.resources().count(), 2);
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "successes"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "failures"));

        let instance = started_instance(
            &composition,
            "onoal.package.test.observability.occurrences.instance",
        );
        let recorder = activate::<DualCounterRecorder>(&instance);
        assert_eq!(
            block_on(recorder.record_success()).expect("success"),
            CounterIncrementResult::Updated { value: 1 }
        );
        assert_eq!(
            block_on(recorder.record_failure()).expect("failure"),
            CounterIncrementResult::Updated { value: 1 }
        );
        assert_eq!(
            block_on(recorder.record_success()).expect("success again"),
            CounterIncrementResult::Updated { value: 2 }
        );
        assert_eq!(
            block_on(recorder.snapshot()).expect("snapshot"),
            DualCounterSnapshot {
                successes: 2,
                failures: 1
            }
        );
    }

    #[test]
    fn repeated_and_multiple_producer_updates_share_one_metric_without_loss() {
        let composition = Fabric::new("onoal.package.test.observability.producers")
            .expect("fabric")
            .with(in_memory_counter("requests"))
            .with(counter_incrementer("requests"))
            .with(counter_batch_incrementer("requests"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.package.test.observability.producers.instance",
        );
        let one = activate::<CounterIncrementer>(&instance);
        let batch = activate::<CounterBatchIncrementer>(&instance);

        assert_eq!(
            block_on(one.increment_by(1)).expect("single"),
            CounterIncrementResult::Updated { value: 1 }
        );
        let batch_result = block_on(batch.increment_many(9, 2)).expect("batch");
        assert_eq!(batch_result.updates.len(), 9);
        assert_eq!(batch_result.final_value, 19);
        assert_eq!(block_on(one.current()).expect("current"), 19);
    }

    #[test]
    fn cross_package_queue_instrumentation_records_actual_outcomes() {
        let composition = Fabric::new("onoal.package.test.observability.queue")
            .expect("fabric")
            .with(fabric_package_messaging_queue::memory_queue("events", 1).expect("queue config"))
            .with(in_memory_counter("successful-sends"))
            .with(in_memory_counter("failed-sends"))
            .with(observed_queue_component())
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.package.test.observability.queue.instance",
        );
        let producer = activate::<ObservedQueueProducer>(&instance);

        assert_eq!(
            block_on(producer.send_observed(b"first".to_vec())).expect("accepted"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(producer.send_observed(b"second".to_vec())).expect("full"),
            QueueSendResult::Full { capacity: 1 }
        );
        assert_eq!(
            block_on(producer.observed_counts()).expect("counts"),
            DualCounterSnapshot {
                successes: 1,
                failures: 1
            }
        );
    }

    #[test]
    fn multi_instance_and_fresh_generation_own_fresh_counter_state() {
        let composition = Fabric::new("onoal.package.test.observability.instances")
            .expect("fabric")
            .with(in_memory_counter("requests"))
            .with(counter_incrementer("requests"))
            .build()
            .expect("composition");
        let mut first = started_instance(
            &composition,
            "onoal.package.test.observability.instances.same",
        );
        let second = started_instance(
            &composition,
            "onoal.package.test.observability.instances.other",
        );

        {
            let first_counter = activate::<CounterIncrementer>(&first);
            assert_eq!(
                block_on(first_counter.increment_by(4)).expect("first"),
                CounterIncrementResult::Updated { value: 4 }
            );
        }
        let second_counter = activate::<CounterIncrementer>(&second);
        assert_eq!(block_on(second_counter.current()).expect("second"), 0);

        first.stop().expect("stop first");
        let stopped_counter = first
            .component::<CounterIncrementer>()
            .expect("stopped counter handle");
        assert!(block_on(stopped_counter.increment_by(1)).is_err());

        let fresh = started_instance(
            &composition,
            "onoal.package.test.observability.instances.same",
        );
        let fresh_counter = activate::<CounterIncrementer>(&fresh);
        assert_eq!(block_on(fresh_counter.current()).expect("fresh"), 0);
    }
}
