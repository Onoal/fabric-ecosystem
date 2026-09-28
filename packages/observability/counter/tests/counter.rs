use fabric::prelude::*;
use fabric_package_observability_counter::{
    in_memory_counter, CounterError, CounterIncrementResult, CounterMetric, InMemoryCounter,
};
use futures::executor::block_on;

fabric::component! {
    TestCounterUser {
        id: "onoal.package.observability.counter.test-user";

        relations {
            requires {
                counter: CounterMetric;
            }
        }

        api {
            fn current(&self) -> Result<u64, CounterError>;
            fn increment_by(&self, amount: u64) -> Result<CounterIncrementResult, CounterError>;
            fn increment_many(&self, times: u64, amount: u64) -> Result<u64, CounterError>;
        }

        runtime {
            fn current(&self) -> Result<u64, CounterError> {
                resolve_resource(self.relations().counter.current())
            }

            fn increment_by(&self, amount: u64) -> Result<CounterIncrementResult, CounterError> {
                resolve_resource(self.relations().counter.increment(amount))
            }

            fn increment_many(&self, times: u64, amount: u64) -> Result<u64, CounterError> {
                for _ in 0..times {
                    match resolve_resource(self.relations().counter.increment(amount))? {
                        CounterIncrementResult::Updated { .. } => {}
                        CounterIncrementResult::Overflow { .. } => break,
                    }
                }
                resolve_resource(self.relations().counter.current())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestCounterSnapshot {
    successes: u64,
    failures: u64,
}

fabric::component! {
    TestDualCounterUser {
        id: "onoal.package.observability.counter.test-dual-user";

        relations {
            requires {
                successes: CounterMetric;
                failures: CounterMetric;
            }
        }

        api {
            fn record_success(&self) -> Result<CounterIncrementResult, CounterError>;
            fn record_failure(&self) -> Result<CounterIncrementResult, CounterError>;
            fn snapshot(&self) -> Result<TestCounterSnapshot, CounterError>;
        }

        runtime {
            fn record_success(&self) -> Result<CounterIncrementResult, CounterError> {
                resolve_resource(self.relations().successes.increment(1))
            }

            fn record_failure(&self) -> Result<CounterIncrementResult, CounterError> {
                resolve_resource(self.relations().failures.increment(1))
            }

            fn snapshot(&self) -> Result<TestCounterSnapshot, CounterError> {
                Ok(TestCounterSnapshot {
                    successes: resolve_resource(self.relations().successes.current())?,
                    failures: resolve_resource(self.relations().failures.current())?,
                })
            }
        }
    }
}

fabric::component! {
    TestConcurrentCounterUser {
        id: "onoal.package.observability.counter.test-concurrent-user";

        relations {
            requires {
                counter: CounterMetric;
            }
        }

        api {
            fn increment_concurrently(&self, workers: usize, per_worker: usize) -> Result<u64, CounterError>;
        }

        runtime {
            fn increment_concurrently(&self, workers: usize, per_worker: usize) -> Result<u64, CounterError> {
                std::thread::scope(|scope| {
                    let mut handles = Vec::new();
                    for _ in 0..workers {
                        handles.push(scope.spawn(|| {
                            for _ in 0..per_worker {
                                match resolve_resource(self.relations().counter.increment(1))? {
                                    CounterIncrementResult::Updated { .. } => {}
                                    CounterIncrementResult::Overflow { .. } => {
                                        return Err(CounterError::increment_failed(
                                            "unexpected overflow in concurrency test",
                                        ));
                                    }
                                }
                            }
                            Ok::<(), CounterError>(())
                        }));
                    }
                    for handle in handles {
                        handle
                            .join()
                            .map_err(|_| CounterError::increment_failed("worker thread panicked"))??;
                    }
                    resolve_resource(self.relations().counter.current())
                })
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

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("counter test resource operation unexpectedly yielded")
        }
    }
}

fn counter_user(counter_name: &'static str) -> impl IntoFabricContribution {
    let counter = CounterMetric::select(counter_name).expect("counter selection");
    let component = TestCounterUser::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("counter").expect("role"),
            fabric::authoring::Requires::<CounterMetric>::provisional(),
        ),
        &counter,
    );
    FabricContribution::new().component(component)
}

fn concurrent_counter_user(counter_name: &'static str) -> impl IntoFabricContribution {
    let counter = CounterMetric::select(counter_name).expect("counter selection");
    let component = TestConcurrentCounterUser::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("counter").expect("role"),
            fabric::authoring::Requires::<CounterMetric>::provisional(),
        ),
        &counter,
    );
    FabricContribution::new().component(component)
}

fn dual_counter_user(
    successes_name: &'static str,
    failures_name: &'static str,
) -> impl IntoFabricContribution {
    let successes = CounterMetric::select(successes_name).expect("success counter");
    let failures = CounterMetric::select(failures_name).expect("failure counter");
    let component = TestDualCounterUser::define()
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
    let helper = Fabric::new("onoal.package.test.observability.counter.inspect.helper")
        .expect("fabric")
        .with(in_memory_counter("requests"))
        .build()
        .expect("composition");
    let explicit = Fabric::new("onoal.package.test.observability.counter.inspect.explicit")
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
fn initial_state_increment_zero_and_overflow_are_semantic_results() {
    let composition = Fabric::new("onoal.package.test.observability.counter.basic")
        .expect("fabric")
        .with(in_memory_counter("requests"))
        .with(counter_user("requests"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.package.test.observability.counter.basic.instance",
    );
    let counter = activate::<TestCounterUser>(&instance);

    assert_eq!(
        block_on(counter.current())
            .expect("fabric call")
            .expect("current"),
        0
    );
    assert_eq!(
        block_on(counter.increment_by(1))
            .expect("fabric call")
            .expect("increment"),
        CounterIncrementResult::Updated { value: 1 }
    );
    assert_eq!(
        block_on(counter.increment_by(4))
            .expect("fabric call")
            .expect("increment"),
        CounterIncrementResult::Updated { value: 5 }
    );
    assert_eq!(
        block_on(counter.increment_by(0))
            .expect("fabric call")
            .expect("zero increment"),
        CounterIncrementResult::Updated { value: 5 }
    );
    assert_eq!(
        block_on(counter.increment_by(u64::MAX - 5))
            .expect("fabric call")
            .expect("max increment"),
        CounterIncrementResult::Updated { value: u64::MAX }
    );
    assert_eq!(
        block_on(counter.increment_by(1))
            .expect("fabric call")
            .expect("overflow"),
        CounterIncrementResult::Overflow {
            current: u64::MAX,
            attempted: 1,
        }
    );
    assert_eq!(
        block_on(counter.current())
            .expect("fabric call")
            .expect("after overflow"),
        u64::MAX
    );
}

#[test]
fn multiple_counter_occurrences_are_independent_by_relation_role() {
    let composition = Fabric::new("onoal.package.test.observability.counter.occurrences")
        .expect("fabric")
        .with(in_memory_counter("successful-operations"))
        .with(in_memory_counter("failed-operations"))
        .with(dual_counter_user(
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
        "onoal.package.test.observability.counter.occurrences.instance",
    );
    let counters = activate::<TestDualCounterUser>(&instance);
    assert_eq!(
        block_on(counters.record_success())
            .expect("fabric call")
            .expect("success"),
        CounterIncrementResult::Updated { value: 1 }
    );
    assert_eq!(
        block_on(counters.record_failure())
            .expect("fabric call")
            .expect("failure"),
        CounterIncrementResult::Updated { value: 1 }
    );
    assert_eq!(
        block_on(counters.record_success())
            .expect("fabric call")
            .expect("success again"),
        CounterIncrementResult::Updated { value: 2 }
    );
    assert_eq!(
        block_on(counters.snapshot())
            .expect("fabric call")
            .expect("snapshot"),
        TestCounterSnapshot {
            successes: 2,
            failures: 1,
        }
    );
}

#[test]
fn consumer_owned_behavior_uses_counter_directly_without_proxy_components() {
    let composition = Fabric::new("onoal.package.test.observability.counter.consumer")
        .expect("fabric")
        .with(in_memory_counter("requests"))
        .with(counter_user("requests"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.package.test.observability.counter.consumer.instance",
    );
    let counter = activate::<TestCounterUser>(&instance);

    assert_eq!(
        block_on(counter.increment_many(9, 2))
            .expect("fabric call")
            .expect("many increments"),
        18
    );
}

#[test]
fn concurrent_in_memory_increments_are_not_lost() {
    let composition = Fabric::new("onoal.package.test.observability.counter.concurrent")
        .expect("fabric")
        .with(in_memory_counter("requests"))
        .with(concurrent_counter_user("requests"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.package.test.observability.counter.concurrent.instance",
    );
    let counter = activate::<TestConcurrentCounterUser>(&instance);

    assert_eq!(
        block_on(counter.increment_concurrently(8, 500))
            .expect("fabric call")
            .expect("concurrent increments"),
        4_000
    );
}

#[test]
fn fresh_generation_owns_fresh_counter_state_and_stopped_instance_fails() {
    let composition = Fabric::new("onoal.package.test.observability.counter.generations")
        .expect("fabric")
        .with(in_memory_counter("requests"))
        .with(counter_user("requests"))
        .build()
        .expect("composition");
    let mut first = started_instance(
        &composition,
        "onoal.package.test.observability.counter.generations.same",
    );
    let second = started_instance(
        &composition,
        "onoal.package.test.observability.counter.generations.other",
    );

    {
        let first_counter = activate::<TestCounterUser>(&first);
        assert_eq!(
            block_on(first_counter.increment_by(4))
                .expect("fabric call")
                .expect("first"),
            CounterIncrementResult::Updated { value: 4 }
        );
    }
    let second_counter = activate::<TestCounterUser>(&second);
    assert_eq!(
        block_on(second_counter.current())
            .expect("fabric call")
            .expect("second"),
        0
    );

    first.stop().expect("stop first");
    let stopped_counter = first
        .component::<TestCounterUser>()
        .expect("stopped counter handle");
    assert!(block_on(stopped_counter.increment_by(1)).is_err());

    let fresh = started_instance(
        &composition,
        "onoal.package.test.observability.counter.generations.same",
    );
    let fresh_counter = activate::<TestCounterUser>(&fresh);
    assert_eq!(
        block_on(fresh_counter.current())
            .expect("fabric call")
            .expect("fresh"),
        0
    );
}
