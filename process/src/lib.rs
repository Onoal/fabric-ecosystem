#![forbid(unsafe_code)]

//! Reusable local operating-system process capability for Fabric compositions.
//!
//! This crate models one narrow capability: a Fabric module that owns a local
//! child process occurrence while a Fabric runtime occurrence is running.

mod definition;
mod error;
mod integration;
mod occurrence;
mod status;

pub use definition::LocalProcessDefinition;
pub use error::{LocalProcessDefinitionError, LocalProcessRuntimeError};
pub use integration::{
    LocalProcessModule, local_process_contract_id, local_process_contract_key, local_process_export,
};
pub use occurrence::LocalProcessHandle;
pub use status::{LocalProcessExit, LocalProcessObservation, LocalProcessStatus};
