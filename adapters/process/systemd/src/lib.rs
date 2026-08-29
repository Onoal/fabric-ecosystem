#![forbid(unsafe_code)]

mod adapter;
mod artifact;
mod config;
mod error;

#[cfg(test)]
mod source_guards;
#[cfg(test)]
mod tests;

pub use adapter::SystemdProcessAdapter;
pub use artifact::{ProcessArtifactResolver, ResolvedProcessArtifact};
pub use config::SystemdProcessConfig;
pub use error::SystemdProcessAdapterError;
