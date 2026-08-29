#![forbid(unsafe_code)]

mod contract;
mod error;
mod http;
mod model;
mod native;

#[cfg(test)]
mod source_guards;
#[cfg(test)]
mod tests;

pub use contract::{ServerContract, ServerService, server_contract_id, server_contract_key};
pub use contract::{
    ServerHttpContract, ServerHttpService, server_http_contract_id, server_http_contract_key,
};
pub use error::ServerError;
pub use http::{
    DispatchServerHttpRequest, ServerHttpHeader, ServerHttpRequest, ServerHttpResponse,
};
pub use model::{
    PreparedServer, ServerArtifact, ServerEntrypoint, ServerId, ServerInstance, ServerInstanceId,
    ServerInstanceStatus, ServerSpec, StartServerRequest, StopServerRequest,
};
pub use native::{
    NativeServer, ServerAdapter, ServerExecution, ServerExecutionRequest, server_resource_id,
};
