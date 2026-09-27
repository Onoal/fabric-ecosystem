//! Text-keyed opaque-value storage package for Fabric.
//!
//! This package owns the `KeyValue` Resource, a package-owned operation error
//! boundary, a generation-local in-memory realization, and authoring helpers.
//! Application behavior owns producer/consumer/client workflows by requiring
//! `KeyValue` directly.

mod authoring;
mod error;
mod memory;
mod resource;

pub use authoring::memory_key_value;
pub use error::{KeyValueError, KeyValueErrorKind};
pub use memory::MemoryKeyValue;
pub use resource::KeyValue;
