use fabric::prelude::*;
use fabric_package_messaging_queue::{
    memory_queue, FifoQueue, InMemoryQueue, InMemoryQueueConfig, QueueConfigError, QueueMessage,
    QueueSendResult,
};
use futures::executor::block_on;

fabric::component! {
    TestQueueProducer {
        id: "onoal.package.messaging.queue.test-producer";

        relations {
            requires {
                outbox: FifoQueue;
            }
        }

        api {
            fn produce(&self, payload: Vec<u8>) -> QueueSendResult;
        }

        runtime {
            fn produce(&self, payload: Vec<u8>) -> QueueSendResult {
                self.relations().outbox.send(payload)
            }
        }
    }
}

fabric::component! {
    TestQueueWorker {
        id: "onoal.package.messaging.queue.test-worker";

        relations {
            requires {
                inbox: FifoQueue;
            }
        }

        api {
            fn process_one(&self) -> Option<Vec<u8>>;
            fn observed_depth(&self) -> usize;
        }

        runtime {
            fn process_one(&self) -> Option<Vec<u8>> {
                self.relations().inbox.try_receive().map(|message| {
                    message
                        .payload
                        .into_iter()
                        .map(|byte| byte.to_ascii_uppercase())
                        .collect()
                })
            }

            fn observed_depth(&self) -> usize {
                self.relations().inbox.depth()
            }
        }
    }
}

fabric::component! {
    TestConsumerA {
        id: "onoal.package.messaging.queue.test-consumer-a";

        relations {
            requires {
                inbox: FifoQueue;
            }
        }

        api {
            fn consume(&self) -> Option<QueueMessage>;
        }

        runtime {
            fn consume(&self) -> Option<QueueMessage> {
                self.relations().inbox.try_receive()
            }
        }
    }
}

fabric::component! {
    TestConsumerB {
        id: "onoal.package.messaging.queue.test-consumer-b";

        relations {
            requires {
                inbox: FifoQueue;
            }
        }

        api {
            fn consume(&self) -> Option<QueueMessage>;
        }

        runtime {
            fn consume(&self) -> Option<QueueMessage> {
                self.relations().inbox.try_receive()
            }
        }
    }
}

fabric::component! {
    TestDualQueueUser {
        id: "onoal.package.messaging.queue.test-dual-user";

        relations {
            requires {
                events: FifoQueue;
                jobs: FifoQueue;
            }
        }

        api {
            fn send_to_both(&self) -> (QueueSendResult, QueueSendResult);
            fn receive_from_both(&self) -> (Option<QueueMessage>, Option<QueueMessage>);
        }

        runtime {
            fn send_to_both(&self) -> (QueueSendResult, QueueSendResult) {
                (
                    self.relations().events.send(b"event".to_vec()),
                    self.relations().jobs.send(b"job".to_vec()),
                )
            }

            fn receive_from_both(&self) -> (Option<QueueMessage>, Option<QueueMessage>) {
                (
                    self.relations().events.try_receive(),
                    self.relations().jobs.try_receive(),
                )
            }
        }
    }
}

fn bind_producer(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("queue selection");
    let component = TestQueueProducer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("outbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

fn bind_worker(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("queue selection");
    let component = TestQueueWorker::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("inbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

fn bind_consumer_a(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("queue selection");
    let component = TestConsumerA::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("inbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

fn bind_consumer_b(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("queue selection");
    let component = TestConsumerB::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("inbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

fn bind_dual_user(
    events_name: &'static str,
    jobs_name: &'static str,
) -> impl IntoFabricContribution {
    let events = FifoQueue::select(events_name).expect("events selection");
    let jobs = FifoQueue::select(jobs_name).expect("jobs selection");
    let component = TestDualQueueUser::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("events").expect("role"),
                fabric::authoring::Requires::<FifoQueue>::provisional(),
            ),
            &events,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("jobs").expect("role"),
                fabric::authoring::Requires::<FifoQueue>::provisional(),
            ),
            &jobs,
        );
    FabricContribution::new().component(component)
}

fn queue_composition(id: &str, capacity: usize) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(memory_queue("jobs", capacity).expect("queue config"))
        .with(bind_producer("jobs"))
        .with(bind_worker("jobs"))
        .build()
        .expect("composition")
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

#[test]
fn composition_declares_queue_and_in_memory_realization() {
    let helper = Fabric::new("onoal.package.test.messaging.inspect.helper")
        .expect("fabric")
        .with(memory_queue("events", 8).expect("queue config"))
        .build()
        .expect("composition");
    let explicit = Fabric::new("onoal.package.test.messaging.inspect.explicit")
        .expect("fabric")
        .resource(
            FifoQueue::select("events")
                .expect("queue")
                .using(InMemoryQueue::new(InMemoryQueueConfig { capacity: 8 }))
                .expect("adapter"),
        )
        .build()
        .expect("composition");

    let helper_queue = helper.resources().next().expect("helper queue");
    let explicit_queue = explicit.resources().next().expect("explicit queue");
    assert_eq!(helper_queue.resource_id(), explicit_queue.resource_id());
    assert_eq!(helper_queue.name(), explicit_queue.name());
    assert_eq!(
        helper_queue
            .realization()
            .adapter_definition_id()
            .expect("helper adapter"),
        explicit_queue
            .realization()
            .adapter_definition_id()
            .expect("explicit adapter")
    );
    assert_eq!(helper.components().count(), 0);
}

#[test]
fn zero_capacity_is_rejected_by_package_authoring() {
    assert!(matches!(
        memory_queue("jobs", 0),
        Err(QueueConfigError::ZeroCapacity)
    ));
}

#[test]
fn producer_worker_fifo_depth_capacity_and_destructive_receive_are_explicit() {
    let composition = queue_composition("onoal.package.test.messaging.fifo", 2);
    let instance = started_instance(&composition, "onoal.package.test.messaging.fifo.instance");
    let producer = activate::<TestQueueProducer>(&instance);
    let worker = activate::<TestQueueWorker>(&instance);

    assert_eq!(block_on(worker.observed_depth()).expect("empty depth"), 0);
    assert_eq!(block_on(worker.process_one()).expect("empty receive"), None);
    assert_eq!(
        block_on(producer.produce(b"first".to_vec())).expect("first send"),
        QueueSendResult::Accepted
    );
    assert_eq!(
        block_on(producer.produce(b"second".to_vec())).expect("second send"),
        QueueSendResult::Accepted
    );
    assert_eq!(block_on(worker.observed_depth()).expect("depth"), 2);
    assert_eq!(
        block_on(producer.produce(b"third".to_vec())).expect("full send"),
        QueueSendResult::Full { capacity: 2 }
    );
    assert_eq!(
        block_on(worker.process_one()).expect("receive first"),
        Some(b"FIRST".to_vec())
    );
    assert_eq!(
        block_on(worker.process_one()).expect("receive second"),
        Some(b"SECOND".to_vec())
    );
    assert_eq!(block_on(worker.process_one()).expect("drained"), None);
    assert_eq!(block_on(worker.observed_depth()).expect("empty depth"), 0);
}

#[test]
fn two_consumer_owned_components_compete_by_destructive_receive() {
    let composition = Fabric::new("onoal.package.test.messaging.competition")
        .expect("fabric")
        .with(memory_queue("jobs", 4).expect("queue config"))
        .with(bind_producer("jobs"))
        .with(bind_consumer_a("jobs"))
        .with(bind_consumer_b("jobs"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.package.test.messaging.competition.instance",
    );
    let producer = activate::<TestQueueProducer>(&instance);
    let first = activate::<TestConsumerA>(&instance);
    let second = activate::<TestConsumerB>(&instance);

    assert_eq!(
        block_on(producer.produce(b"from-a".to_vec())).expect("send a"),
        QueueSendResult::Accepted
    );
    assert_eq!(
        block_on(producer.produce(b"from-b".to_vec())).expect("send b"),
        QueueSendResult::Accepted
    );
    assert_eq!(
        block_on(first.consume()).expect("first receive"),
        Some(QueueMessage {
            payload: b"from-a".to_vec()
        })
    );
    assert_eq!(
        block_on(second.consume()).expect("second receive"),
        Some(QueueMessage {
            payload: b"from-b".to_vec()
        })
    );
    assert_eq!(block_on(first.consume()).expect("queue empty"), None);
}

#[test]
fn multiple_queue_occurrences_do_not_leak() {
    let composition = Fabric::new("onoal.package.test.messaging.occurrences")
        .expect("fabric")
        .with(memory_queue("events", 2).expect("events queue config"))
        .with(memory_queue("jobs", 2).expect("jobs queue config"))
        .with(bind_dual_user("events", "jobs"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.package.test.messaging.occurrences.instance",
    );
    let user = activate::<TestDualQueueUser>(&instance);

    assert_eq!(
        block_on(user.send_to_both()).expect("send both"),
        (QueueSendResult::Accepted, QueueSendResult::Accepted)
    );
    assert_eq!(
        block_on(user.receive_from_both()).expect("receive both"),
        (
            Some(QueueMessage {
                payload: b"event".to_vec()
            }),
            Some(QueueMessage {
                payload: b"job".to_vec()
            })
        )
    );
}

#[test]
fn in_memory_state_is_instance_local_and_fresh_generation_local() {
    let composition = queue_composition("onoal.package.test.messaging.instances", 4);
    let mut first = started_instance(&composition, "onoal.package.test.messaging.instances.same");
    let second = started_instance(&composition, "onoal.package.test.messaging.instances.other");

    let first_producer = activate::<TestQueueProducer>(&first);
    let first_worker = activate::<TestQueueWorker>(&first);
    let second_worker = activate::<TestQueueWorker>(&second);

    assert_eq!(
        block_on(first_producer.produce(b"first-instance".to_vec())).expect("send"),
        QueueSendResult::Accepted
    );
    assert_eq!(
        block_on(second_worker.process_one()).expect("second empty"),
        None
    );
    assert_eq!(
        block_on(first_worker.process_one()).expect("first receives"),
        Some(b"FIRST-INSTANCE".to_vec())
    );

    assert_eq!(
        block_on(first_producer.produce(b"generation-local".to_vec())).expect("send"),
        QueueSendResult::Accepted
    );
    first.stop().expect("stop first");

    let fresh = started_instance(&composition, "onoal.package.test.messaging.instances.same");
    let fresh_worker = activate::<TestQueueWorker>(&fresh);
    assert_eq!(
        block_on(fresh_worker.process_one()).expect("fresh empty"),
        None
    );
}

#[test]
fn stopped_instance_does_not_fabricate_successful_messaging() {
    let composition = queue_composition("onoal.package.test.messaging.stopped", 2);
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.messaging.stopped.instance",
    );
    {
        let producer = activate::<TestQueueProducer>(&instance);
        assert_eq!(
            block_on(producer.produce(b"before-stop".to_vec())).expect("send"),
            QueueSendResult::Accepted
        );
    }

    instance.stop().expect("stop instance");
    let producer = instance.component::<TestQueueProducer>().expect("producer");
    assert!(block_on(producer.produce(b"after-stop".to_vec())).is_err());
}
