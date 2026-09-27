//! Monotonic counter metric capability for Fabric.
//!
//! This package owns one deliberately small observability capability:
//! `CounterMetric`, a named monotonic unsigned counter. It also provides a
//! generation-local in-memory realization and authoring helper. Application
//! behavior owns what a counter means and when it is incremented.

mod authoring;
mod error;
mod memory;
mod model;
mod resource;

pub use authoring::in_memory_counter;
pub use error::{CounterError, CounterErrorKind};
pub use memory::InMemoryCounter;
pub use model::CounterIncrementResult;
pub use resource::CounterMetric;
