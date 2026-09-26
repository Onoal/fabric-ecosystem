//! Deterministic example for extending the Local Queue Pipeline Composition.

use fabric::prelude::*;
use fabric_composition_local_queue_pipeline::{local_queue_pipeline, LocalQueuePipelineConfig};
use fabric_package_messaging_queue::{
    FifoQueue, QueueProducer, QueueProducerInstanceApi, QueueSendResult,
};
use futures::executor::block_on;

fabric::component! {
    ExampleQueueWorker {
        id: "onoal.example.local-queue-pipeline.worker";

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
                    .map(|message| {
                        let mut output = b"processed:".to_vec();
                        output.extend(message.payload);
                        output
                    })
            }
        }
    }
}

pub fn example_worker(queue_name: &'static str) -> impl IntoFabricContribution {
    let queue = FifoQueue::select(queue_name).expect("queue selection");
    let component = ExampleQueueWorker::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("jobs").expect("role"),
            fabric::authoring::Requires::<FifoQueue>::provisional(),
        ),
        &queue,
    );
    FabricContribution::new().component(component)
}

pub fn run_example() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let composition = Fabric::new("onoal.example.local-queue-pipeline")?
        .with(local_queue_pipeline(LocalQueuePipelineConfig::new(
            "jobs", 8,
        )?))
        .with(example_worker("jobs"))
        .build()?;
    let mut instance = composition.materialize_on(
        "onoal.example.local-queue-pipeline.instance",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let producer = instance.component::<QueueProducer>()?;
    producer.reconcile()?;
    let worker = instance.component::<ExampleQueueWorker>()?;
    worker.reconcile()?;

    assert_eq!(
        block_on(producer.produce(b"compile-docs".to_vec()))?,
        QueueSendResult::Accepted
    );
    Ok(block_on(worker.process_one())?.expect("processed message"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_extends_pipeline_with_consumer_owned_worker() {
        assert_eq!(run_example().expect("example"), b"processed:compile-docs");
    }
}
