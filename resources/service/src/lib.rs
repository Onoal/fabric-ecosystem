#![forbid(unsafe_code)]

mod contract;
mod error;
mod model;
mod native;
mod runtime;

#[cfg(test)]
mod tests;

pub use contract::{ServiceContract, ServiceService, service_contract_id, service_contract_key};
pub use error::ServiceError;
pub use model::{
    MarkServiceTargetDrainingRequest, MarkServiceTargetReadyRequest, PreparedService,
    RegisterServiceTargetRequest, Service, ServiceEndpoint, ServiceEndpointId, ServiceHttpHeader,
    ServiceHttpRequest, ServiceHttpResponse, ServiceId, ServiceLiveEndpoint, ServiceName,
    ServiceProtocol, ServiceRequirement, ServiceScope, ServiceTarget, ServiceTargetId,
    ServiceTargetState, WithdrawServiceTargetRequest,
};
pub use native::{NativeServices, NativeServicesConfig};
pub use runtime::{ServiceHttpTargetRuntime, ServiceHttpTargetRuntimeService};
