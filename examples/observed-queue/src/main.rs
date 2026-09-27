mod app;

use std::io::Error;

use app::{ObservedQueueProducer, ObservedQueueProducerInstanceApi};
use fabric::prelude::*;
use fabric_package_messaging_queue::QueueSendResult;
use fabric_package_observability_counter::in_memory_counter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.observed-queue")?
        .with(fabric_package_messaging_queue::memory_queue("events", 1)?)
        .with(in_memory_counter("successful-sends"))
        .with(in_memory_counter("failed-sends"))
        .with(app::observed_queue_producer())
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.observed-queue.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let producer = instance.component::<ObservedQueueProducer>()?;
    producer.reconcile()?;
    let first = futures::executor::block_on(producer.send_observed(b"first".to_vec()))??;
    if first != QueueSendResult::Accepted {
        return Err(Box::new(Error::other(
            "first queue send was expected to be accepted",
        )));
    }
    let second = futures::executor::block_on(producer.send_observed(b"second".to_vec()))??;
    if second != (QueueSendResult::Full { capacity: 1 }) {
        return Err(Box::new(Error::other(
            "second queue send was expected to observe full capacity",
        )));
    }
    let counts = futures::executor::block_on(producer.observed_counts())??;

    instance.stop()?;

    println!("first send: accepted");
    println!("second send: full at capacity 1");
    println!("successful sends: {}", counts.successes);
    println!("failed sends: {}", counts.failures);

    Ok(())
}
