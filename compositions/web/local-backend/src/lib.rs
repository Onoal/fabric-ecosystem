//! Reusable local backend foundation Composition artifact.
//!
//! This crate owns one reusable local backend assembly opinion. It nests the
//! HTTP server Composition and adds SQLite and console logging realizations. It
//! defines no production Fabric Resource, System, Component, or Adapter.

mod assembly;
mod config;

pub use assembly::local_backend_stack;
pub use config::LocalBackendCompositionConfig;
