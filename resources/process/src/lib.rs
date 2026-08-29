#![forbid(unsafe_code)]

mod contract;
mod error;
mod model;
mod native;

#[cfg(test)]
mod source_guards;
#[cfg(test)]
mod tests;

pub use contract::{ProcessContract, ProcessService, process_contract_id, process_contract_key};
pub use error::ProcessError;
pub use model::{
    PreparedProcess, PreparedProcessEnvironment, ProcessArtifact, ProcessEntrypoint, ProcessId,
    ProcessInstance, ProcessInstanceId, ProcessInstanceStatus, ProcessSpec, StartProcessRequest,
    StopProcessRequest,
};
pub use native::{
    NativeProcess, ProcessAdapter, ProcessExecution, ProcessExecutionRequest, process_resource_id,
};
