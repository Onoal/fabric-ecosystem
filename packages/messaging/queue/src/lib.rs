//! Bounded non-durable FIFO queue package for Fabric.
//!
//! This package owns the `FifoQueue` Resource, an in-memory realization, and
//! queue authoring helpers. Application code owns producer, consumer, and
//! worker behavior by requiring `FifoQueue` directly.

mod authoring;
mod memory;
mod model;
mod resource;

pub use authoring::{memory_queue, QueueConfigError};
pub use memory::{InMemoryQueue, InMemoryQueueConfig};
pub use model::{QueueMessage, QueueSendResult};
pub use resource::FifoQueue;
