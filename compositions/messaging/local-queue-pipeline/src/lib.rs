//! Reusable local queue pipeline Composition artifact.
//!
//! This crate composes existing Queue package authoring into ordinary Fabric
//! Composition truth. It defines no new production Fabric Resource, System,
//! Component, or Adapter.

use fabric::prelude::*;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalQueuePipelineConfig {
    queue_name: &'static str,
    capacity: usize,
}

impl LocalQueuePipelineConfig {
    pub fn new(
        queue_name: &'static str,
        capacity: usize,
    ) -> Result<Self, LocalQueuePipelineConfigError> {
        if capacity == 0 {
            return Err(LocalQueuePipelineConfigError::ZeroCapacity);
        }
        Ok(Self {
            queue_name,
            capacity,
        })
    }

    pub fn queue_name(&self) -> &'static str {
        self.queue_name
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalQueuePipelineConfigError {
    ZeroCapacity,
}

impl fmt::Display for LocalQueuePipelineConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => write!(
                formatter,
                "local queue pipeline capacity must be greater than zero"
            ),
        }
    }
}

impl Error for LocalQueuePipelineConfigError {}

pub fn local_queue_pipeline(config: LocalQueuePipelineConfig) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(fabric_package_messaging_queue::memory_queue(
            config.queue_name,
            config.capacity,
        ))
        .with(fabric_package_messaging_queue::queue_producer(
            config.queue_name,
        ))
        .with(fabric_package_messaging_queue::queue_consumer(
            config.queue_name,
        ))
}

pub fn build_local_queue_pipeline_composition(
    id: &str,
    config: LocalQueuePipelineConfig,
) -> Result<Composition, Box<dyn Error>> {
    Fabric::new(id)
        .map_err(|error| Box::new(error) as Box<dyn Error>)?
        .with(local_queue_pipeline(config))
        .build()
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_package_messaging_queue::{
        competing_queue_consumer, memory_queue, CompetingQueueConsumer,
        CompetingQueueConsumerInstanceApi, FifoQueue, QueueConsumer, QueueConsumerInstanceApi,
        QueueMessage, QueueProducer, QueueProducerInstanceApi, QueueSendResult,
    };
    use futures::executor::block_on;

    fabric::component! {
        TestQueueWorker {
            id: "onoal.composition.test.local-queue-pipeline.worker";

            relations {
                requires {
                    jobs: FifoQueue;
                }
            }

            api {
                fn process_one(&self) -> Option<Vec<u8>>;
            }

            runtime {
                fn process_one(&self) -> Option<Vec<u8>> {
                    self.relations()
                        .jobs
                        .try_receive()
                        .map(|message| message.payload.into_iter().map(|byte| byte.to_ascii_uppercase()).collect())
                }
            }
        }
    }

    fn config(name: &'static str, capacity: usize) -> LocalQueuePipelineConfig {
        LocalQueuePipelineConfig::new(name, capacity).expect("valid config")
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

    fn worker(queue_name: &'static str) -> impl IntoFabricContribution {
        let queue = FifoQueue::select(queue_name).expect("queue selection");
        let component = TestQueueWorker::define().select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("jobs").expect("role"),
                fabric::authoring::Requires::<FifoQueue>::provisional(),
            ),
            &queue,
        );
        FabricContribution::new().component(component)
    }

    #[test]
    fn config_rejects_zero_capacity() {
        assert_eq!(
            LocalQueuePipelineConfig::new("jobs", 0),
            Err(LocalQueuePipelineConfigError::ZeroCapacity)
        );
    }

    #[test]
    fn standalone_composition_declares_expected_queue_pipeline_graph() {
        let composition = build_local_queue_pipeline_composition(
            "onoal.composition.test.local-queue-pipeline.inspect",
            config("jobs", 8),
        )
        .expect("composition");

        assert_eq!(composition.resources().count(), 1);
        assert_eq!(composition.components().count(), 2);
        let queue = composition.resources().next().expect("queue");
        assert_eq!(
            queue.resource_id().as_str(),
            "onoal.package.messaging.queue.fifo"
        );
        assert_eq!(queue.name().as_str(), "jobs");
        assert_eq!(
            queue
                .realization()
                .adapter_definition_id()
                .expect("in-memory adapter")
                .as_str(),
            "onoal.package.messaging.queue.fifo.in-memory"
        );
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.messaging.queue.producer"));
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.messaging.queue.consumer"));
        assert!(composition.relations().iter().any(|relation| {
            relation.role().as_str() == "outbox"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "jobs"
                )
        }));
        assert!(composition.relations().iter().any(|relation| {
            relation.role().as_str() == "inbox"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "jobs"
                )
        }));
    }

    #[test]
    fn fifo_runtime_capacity_and_depth_are_preserved() {
        let composition = build_local_queue_pipeline_composition(
            "onoal.composition.test.local-queue-pipeline.runtime",
            config("jobs", 2),
        )
        .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.local-queue-pipeline.runtime.instance",
        );
        let producer = activate::<QueueProducer>(&instance);
        let consumer = activate::<QueueConsumer>(&instance);

        assert_eq!(block_on(consumer.consume()).expect("empty"), None);
        assert_eq!(block_on(consumer.observed_depth()).expect("depth"), 0);
        assert_eq!(
            block_on(producer.produce(b"first".to_vec())).expect("first"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(producer.produce(b"second".to_vec())).expect("second"),
            QueueSendResult::Accepted
        );
        assert_eq!(block_on(consumer.observed_depth()).expect("depth"), 2);
        assert_eq!(
            block_on(producer.produce(b"third".to_vec())).expect("full"),
            QueueSendResult::Full { capacity: 2 }
        );
        assert_eq!(
            block_on(consumer.consume()).expect("consume first"),
            Some(QueueMessage {
                payload: b"first".to_vec()
            })
        );
        assert_eq!(
            block_on(producer.produce(b"third".to_vec())).expect("space"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(consumer.consume()).expect("consume second"),
            Some(QueueMessage {
                payload: b"second".to_vec()
            })
        );
        assert_eq!(
            block_on(consumer.consume()).expect("consume third"),
            Some(QueueMessage {
                payload: b"third".to_vec()
            })
        );
        assert_eq!(block_on(consumer.consume()).expect("drained"), None);
        assert_eq!(block_on(consumer.observed_depth()).expect("depth"), 0);
    }

    #[test]
    fn fresh_generation_starts_empty_because_pipeline_is_non_durable() {
        let composition = build_local_queue_pipeline_composition(
            "onoal.composition.test.local-queue-pipeline.generation",
            config("jobs", 4),
        )
        .expect("composition");
        let mut first = started_instance(
            &composition,
            "onoal.composition.test.local-queue-pipeline.generation.same",
        );
        let first_producer = activate::<QueueProducer>(&first);
        assert_eq!(
            block_on(first_producer.produce(b"generation-local".to_vec())).expect("send"),
            QueueSendResult::Accepted
        );
        first.stop().expect("stop");

        let fresh = started_instance(
            &composition,
            "onoal.composition.test.local-queue-pipeline.generation.same",
        );
        let fresh_consumer = activate::<QueueConsumer>(&fresh);
        assert_eq!(block_on(fresh_consumer.consume()).expect("fresh"), None);
    }

    #[test]
    fn consumer_owned_worker_extends_pipeline_behavior() {
        let composition = Fabric::new("onoal.composition.test.local-queue-pipeline.worker")
            .expect("fabric")
            .with(local_queue_pipeline(config("jobs", 4)))
            .with(worker("jobs"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.local-queue-pipeline.worker.instance",
        );
        let producer = activate::<QueueProducer>(&instance);
        let worker = activate::<TestQueueWorker>(&instance);

        assert_eq!(
            block_on(producer.produce(b"hello".to_vec())).expect("send"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(worker.process_one()).expect("process"),
            Some(b"HELLO".to_vec())
        );
    }

    #[test]
    fn competing_consumer_can_be_added_by_external_authoring() {
        let composition = Fabric::new("onoal.composition.test.local-queue-pipeline.competing")
            .expect("fabric")
            .with(local_queue_pipeline(config("jobs", 4)))
            .with(competing_queue_consumer("jobs"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.local-queue-pipeline.competing.instance",
        );
        let producer = activate::<QueueProducer>(&instance);
        let first = activate::<QueueConsumer>(&instance);
        let second = activate::<CompetingQueueConsumer>(&instance);

        assert_eq!(
            block_on(producer.produce(b"first".to_vec())).expect("first"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(producer.produce(b"second".to_vec())).expect("second"),
            QueueSendResult::Accepted
        );
        assert_eq!(
            block_on(first.consume()).expect("first consumer"),
            Some(QueueMessage {
                payload: b"first".to_vec()
            })
        );
        assert_eq!(
            block_on(second.consume()).expect("second consumer"),
            Some(QueueMessage {
                payload: b"second".to_vec()
            })
        );
    }

    #[test]
    fn configured_queue_name_is_preserved() {
        let composition = build_local_queue_pipeline_composition(
            "onoal.composition.test.local-queue-pipeline.naming",
            config("events", 8),
        )
        .expect("composition");

        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "events"));
        assert!(composition.relations().iter().any(|relation| {
            relation.role().as_str() == "outbox"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "events"
                )
        }));
        assert!(composition.relations().iter().any(|relation| {
            relation.role().as_str() == "inbox"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "events"
                )
        }));
    }

    #[test]
    fn multiple_queue_resources_can_coexist_without_pipeline_components() {
        let composition = Fabric::new("onoal.composition.test.local-queue-pipeline.resources")
            .expect("fabric")
            .with(memory_queue("jobs", 2))
            .with(memory_queue("events", 2))
            .build()
            .expect("composition");

        assert_eq!(composition.resources().count(), 2);
        assert_eq!(composition.components().count(), 0);
        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "jobs"));
        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "events"));
    }

    #[test]
    fn multiple_full_pipelines_hit_current_component_identity_law() {
        Fabric::new("onoal.composition.test.local-queue-pipeline.multi")
            .expect("fabric")
            .with(local_queue_pipeline(config("jobs", 2)))
            .with(local_queue_pipeline(config("events", 2)))
            .build()
            .expect_err("duplicate QueueProducer/QueueConsumer component definitions");
    }
}
