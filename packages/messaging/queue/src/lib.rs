//! FIFO queue messaging package for Fabric.
//!
//! This package models one bounded, non-durable FIFO queue capability. Receive
//! is destructive and non-blocking; multiple consumers compete for messages.

use std::collections::VecDeque;
use std::sync::Mutex;

use fabric::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueMessage {
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueSendResult {
    Accepted,
    Full { capacity: usize },
}

#[derive(Default)]
pub struct InMemoryQueueState {
    messages: Mutex<VecDeque<QueueMessage>>,
}

fabric::resource! {
    pub FifoQueue {
        id: "onoal.package.messaging.queue.fifo";

        api {
            fn send(&self, payload: Vec<u8>) -> QueueSendResult;
            fn try_receive(&self) -> Option<QueueMessage>;
            fn depth(&self) -> usize;
        }
    }
}

fabric::adapter! {
    pub InMemoryQueue for FifoQueue {
        id: "onoal.package.messaging.queue.fifo.in-memory";

        config {
            capacity: usize;
        }

        state {
            InMemoryQueueState = InMemoryQueueState::default();
        }

        runtime {
            fn send(&self, payload: Vec<u8>) -> QueueSendResult {
                let mut messages = self
                    .state
                    .get()
                    .messages
                    .lock()
                    .expect("in-memory queue state");
                if messages.len() >= self.config.capacity {
                    QueueSendResult::Full {
                        capacity: self.config.capacity,
                    }
                } else {
                    messages.push_back(QueueMessage { payload });
                    QueueSendResult::Accepted
                }
            }

            fn try_receive(&self) -> Option<QueueMessage> {
                self.state
                    .get()
                    .messages
                    .lock()
                    .expect("in-memory queue state")
                    .pop_front()
            }

            fn depth(&self) -> usize {
                self.state
                    .get()
                    .messages
                    .lock()
                    .expect("in-memory queue state")
                    .len()
            }
        }
    }
}

fabric::component! {
    pub QueueProducer {
        id: "onoal.package.messaging.queue.producer";

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
    pub QueueConsumer {
        id: "onoal.package.messaging.queue.consumer";

        relations {
            requires {
                inbox: FifoQueue;
            }
        }

        api {
            fn consume(&self) -> Option<QueueMessage>;
            fn observed_depth(&self) -> usize;
        }

        runtime {
            fn consume(&self) -> Option<QueueMessage> {
                self.relations().inbox.try_receive()
            }

            fn observed_depth(&self) -> usize {
                self.relations().inbox.depth()
            }
        }
    }
}

fabric::component! {
    pub CompetingQueueConsumer {
        id: "onoal.package.messaging.queue.competing-consumer";

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
    pub DualQueueProbe {
        id: "onoal.package.messaging.queue.dual-probe";

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

pub fn memory_queue(name: &'static str, capacity: usize) -> impl IntoFabricContribution {
    let selected = FifoQueue::select(name).expect("valid queue resource name");
    FabricContribution::new().resource(
        selected
            .using(InMemoryQueue::new(InMemoryQueueConfig { capacity }))
            .expect("InMemoryQueue supports FifoQueue"),
    )
}

pub fn queue_producer(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("valid queue resource name");
    let component = QueueProducer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("outbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

pub fn queue_consumer(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("valid queue resource name");
    let component = QueueConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("inbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

pub fn competing_queue_consumer(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("valid queue resource name");
    let component = CompetingQueueConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("inbox").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

pub fn dual_queue_probe(
    events_name: &'static str,
    jobs_name: &'static str,
) -> impl IntoFabricContribution {
    let events = FifoQueue::select(events_name).expect("valid events queue resource name");
    let jobs = FifoQueue::select(jobs_name).expect("valid jobs queue resource name");
    let component = DualQueueProbe::define()
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    fn messaging_composition(id: &str, capacity: usize) -> Composition {
        Fabric::new(id)
            .expect("fabric")
            .with(memory_queue("events", capacity))
            .with(queue_producer("events"))
            .with(queue_consumer("events"))
            .with(competing_queue_consumer("events"))
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
            .with(memory_queue("events", 8))
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
    fn producer_consumer_transfer_fifo_order_empty_and_capacity_are_explicit() {
        let composition = messaging_composition("onoal.package.test.messaging.fifo", 2);
        let instance = started_instance(&composition, "onoal.package.test.messaging.fifo.instance");
        let producer = activate::<QueueProducer>(&instance);
        let consumer = activate::<QueueConsumer>(&instance);

        assert_eq!(block_on(consumer.consume()).expect("empty receive"), None);
        assert_eq!(
            block_on(producer.produce(b"first".to_vec())).expect("first send"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(producer.produce(b"second".to_vec())).expect("second send"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(producer.produce(b"third".to_vec())).expect("full send"),
            QueueSendResult::Full { capacity: 2 }
        );
        assert_eq!(
            block_on(consumer.consume()).expect("receive first"),
            Some(QueueMessage {
                payload: b"first".to_vec()
            })
        );
        assert_eq!(
            block_on(consumer.consume()).expect("receive second"),
            Some(QueueMessage {
                payload: b"second".to_vec()
            })
        );
        assert_eq!(block_on(consumer.consume()).expect("drained"), None);
    }

    #[test]
    fn multiple_producer_interactions_and_competing_consumers_share_one_fifo() {
        let composition = messaging_composition("onoal.package.test.messaging.competition", 4);
        let instance = started_instance(
            &composition,
            "onoal.package.test.messaging.competition.instance",
        );
        let producer = activate::<QueueProducer>(&instance);
        let first_consumer = activate::<QueueConsumer>(&instance);
        let second_consumer = activate::<CompetingQueueConsumer>(&instance);

        assert_eq!(
            block_on(producer.produce(b"from-a".to_vec())).expect("send a"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(producer.produce(b"from-b".to_vec())).expect("send b"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(first_consumer.consume()).expect("first competing receive"),
            Some(QueueMessage {
                payload: b"from-a".to_vec()
            })
        );
        assert_eq!(
            block_on(second_consumer.consume()).expect("second competing receive"),
            Some(QueueMessage {
                payload: b"from-b".to_vec()
            })
        );
        assert_eq!(
            block_on(first_consumer.consume()).expect("queue now empty"),
            None
        );
    }

    #[test]
    fn multiple_queue_occurrences_do_not_leak() {
        let composition = Fabric::new("onoal.package.test.messaging.occurrences")
            .expect("fabric")
            .with(memory_queue("events", 2))
            .with(memory_queue("jobs", 2))
            .with(dual_queue_probe("events", "jobs"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.package.test.messaging.occurrences.instance",
        );
        let probe = activate::<DualQueueProbe>(&instance);

        assert_eq!(
            block_on(probe.send_to_both()).expect("send both"),
            (QueueSendResult::Accepted, QueueSendResult::Accepted)
        );
        assert_eq!(
            block_on(probe.receive_from_both()).expect("receive both"),
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
        let composition = messaging_composition("onoal.package.test.messaging.instances", 4);
        let mut first =
            started_instance(&composition, "onoal.package.test.messaging.instances.same");
        let second = started_instance(&composition, "onoal.package.test.messaging.instances.other");

        let first_producer = activate::<QueueProducer>(&first);
        let first_consumer = activate::<QueueConsumer>(&first);
        let second_consumer = activate::<QueueConsumer>(&second);

        assert_eq!(
            block_on(first_producer.produce(b"first-instance".to_vec())).expect("send"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(second_consumer.consume()).expect("second empty"),
            None
        );
        assert_eq!(
            block_on(first_consumer.consume()).expect("first receives"),
            Some(QueueMessage {
                payload: b"first-instance".to_vec()
            })
        );

        assert_eq!(
            block_on(first_producer.produce(b"generation-local".to_vec())).expect("send"),
            QueueSendResult::Accepted
        );
        first.stop().expect("stop first");

        let fresh = started_instance(&composition, "onoal.package.test.messaging.instances.same");
        let fresh_consumer = activate::<QueueConsumer>(&fresh);
        assert_eq!(
            block_on(fresh_consumer.consume()).expect("fresh empty"),
            None
        );
    }

    #[test]
    fn stopped_instance_does_not_fabricate_successful_messaging() {
        let composition = messaging_composition("onoal.package.test.messaging.stopped", 2);
        let mut instance = started_instance(
            &composition,
            "onoal.package.test.messaging.stopped.instance",
        );
        {
            let producer = activate::<QueueProducer>(&instance);
            assert_eq!(
                block_on(producer.produce(b"before-stop".to_vec())).expect("send"),
                QueueSendResult::Accepted
            );
        }

        instance.stop().expect("stop instance");
        let producer = instance.component::<QueueProducer>().expect("producer");
        assert!(block_on(producer.produce(b"after-stop".to_vec())).is_err());
    }
}
