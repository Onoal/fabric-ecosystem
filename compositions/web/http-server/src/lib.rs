//! Reusable local HTTP server Composition artifact.
//!
//! This crate owns one reusable assembly opinion over existing TCP and HTTP
//! packages. It defines no new Fabric Resource, System, Component, Adapter, or
//! runtime policy.

mod assembly;
mod config;

pub use assembly::{http_server_stack, local_http_server};
pub use config::HttpServerCompositionConfig;
