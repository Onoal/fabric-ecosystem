#![forbid(unsafe_code)]

mod contract;
mod error;
mod local_access;
mod model;
mod native;

#[cfg(test)]
mod tests;

pub use contract::{
    ConnectivityContract, ConnectivityService, connectivity_contract_id, connectivity_contract_key,
    connectivity_resource_id,
};
pub use error::ConnectivityError;
pub use local_access::{
    LocalConnectivityAccess, LocalConnectivityAccessContract, LocalConnectivityAccessService,
    local_connectivity_access_contract_id, local_connectivity_access_contract_key,
};
pub use model::{ConnectivityScope, PreparedReachability, Reachability, ReachabilityId};
pub use native::{NativeConnectivity, NativeConnectivityConfig};
