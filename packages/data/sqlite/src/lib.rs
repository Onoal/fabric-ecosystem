//! SQLite realization package for Fabric RelationalDatabase.
//!
//! The `RelationalDatabase` Resource is owned by
//! `onoal-fabric-package-relational-database`. This crate supplies a concrete
//! SQLite Adapter, SQLite-owned configuration, conversion, lifecycle, and
//! authoring helpers.

mod adapter;
mod authoring;
mod config;
mod conversion;
mod error;
mod state;

pub use adapter::SqliteRelationalDatabase;
pub use authoring::{in_memory_sqlite_database, sqlite_database, sqlite_database_with};
pub use config::SqliteDatabasePath;
