#![forbid(unsafe_code)]

mod bindings;
mod compatibility;
mod contract;
mod error;
mod model;
mod native;
mod projection;

#[cfg(test)]
mod source_guards;
#[cfg(test)]
mod tests;

pub use fabric_binding::BindingName;

pub use bindings::{
    BindingProjection, BindingTarget, WorkloadBinding, WorkloadBindingEnv,
    WorkloadBindingProjection, validate_workload_bindings,
};
pub use compatibility::{CompatibilityIssue, CompatibilityReport};
pub use contract::{WorkerContract, WorkerService, worker_contract_id, worker_contract_key};
pub use contract::{
    WorkerHttpContract, WorkerHttpService, worker_http_contract_id, worker_http_contract_key,
};
pub use error::WorkerError;
pub use model::WorkloadRequirement as WorkerRequirement;
pub use model::{
    DispatchHttpRequest, HttpHeader, HttpRequest, HttpResponse, PreparedWorker, StartWorkerRequest,
    StopWorkerRequest, WorkerApiVersion, WorkerCapabilities, WorkerFeature, WorkerFeatureSupport,
    WorkerInstance, WorkerInstanceId, WorkerInstanceStatus, WorkerSpec, WorkloadArtifact,
    WorkloadEntrypoint, WorkloadId, WorkloadRequirement,
};
pub use native::{
    NativeWorker, WorkerAdapter, WorkerExecution, WorkerExecutionRequest, worker_resource_id,
};
pub use projection::{
    NativeWorkloadProjection, PreparedWorkloadEnvironmentValue, PreparedWorkloadProjections,
    SensitiveProjectionValue, WorkloadProjectionContract, WorkloadProjectionService,
    workload_projection_contract_id, workload_projection_contract_key,
};
