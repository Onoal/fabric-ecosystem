use std::error::Error;
use std::fmt;

use fabric::prelude::*;

use crate::{FifoQueue, InMemoryQueue, InMemoryQueueConfig};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueConfigError {
    ZeroCapacity,
}

impl fmt::Display for QueueConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => write!(f, "in-memory FIFO queue capacity must be greater than 0"),
        }
    }
}

impl Error for QueueConfigError {}

pub fn memory_queue(
    name: &'static str,
    capacity: usize,
) -> Result<FabricContribution, QueueConfigError> {
    if capacity == 0 {
        return Err(QueueConfigError::ZeroCapacity);
    }
    let selected = FifoQueue::select(name).expect("valid queue resource name");
    Ok(FabricContribution::new().resource(
        selected
            .using(InMemoryQueue::new(InMemoryQueueConfig { capacity }))
            .expect("InMemoryQueue supports FifoQueue"),
    ))
}
