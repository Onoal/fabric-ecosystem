//! Run-to-completion local process execution package for Fabric.
//!
//! This package owns the `ProcessRuntime` Resource, invocation/output/error
//! models, a local OS-process realization, and authoring helpers. It is not a
//! process supervisor, daemon manager, worker runtime, or deployment system.

mod authoring;
mod error;
mod invocation;
mod local;
mod output;
mod resource;

pub use authoring::local_process_runtime;
pub use error::{ProcessExecutionError, ProcessExecutionErrorKind};
pub use invocation::{ProcessEnvironmentVariable, ProcessInvocation};
pub use local::LocalProcessRuntime;
pub use output::ProcessOutput;
pub use resource::ProcessRuntime;
