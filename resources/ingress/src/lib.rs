#![forbid(unsafe_code)]

mod contract;
mod error;
mod local_access;
mod model;
mod native;

#[cfg(test)]
mod tests;

pub use contract::{
    IngressContract, IngressService, ingress_contract_id, ingress_contract_key, ingress_resource_id,
};
pub use error::IngressError;
pub use local_access::LocalHttpIngressAccess;
pub use local_access::{
    LocalHttpIngressAccessContract, LocalHttpIngressAccessService,
    local_http_ingress_access_contract_id, local_http_ingress_access_contract_key,
};
pub use model::{IngressRoute, IngressRouteId, IngressRouteTarget};
pub use native::{IngressAdapter, NativeIngress};
