//! Portable relational database semantic package for Fabric.
//!
//! This package owns the generic `RelationalDatabase` Resource and its
//! portable invocation, value, result, and operation-error shape. Concrete
//! backends such as SQLite belong in realization packages.

mod error;
mod model;
mod resource;

pub use error::{RelationalDatabaseError, RelationalDatabaseErrorKind};
pub use model::{RelationalQueryResult, RelationalRow, RelationalValue};
pub use resource::RelationalDatabase;
