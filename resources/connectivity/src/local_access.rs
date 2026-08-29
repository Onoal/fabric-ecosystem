use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};

use crate::{ConnectivityError, ReachabilityId};

const LOCAL_CONNECTIVITY_ACCESS_CONTRACT_ID: &str = "fabric.resource.connectivity.local-access";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalConnectivityAccess {
    pub reachability_id: ReachabilityId,
    pub url: String,
}

pub fn local_connectivity_access_contract_id() -> ContractId {
    ContractId::new(LOCAL_CONNECTIVITY_ACCESS_CONTRACT_ID)
        .expect("static local connectivity access contract id")
}

pub fn local_connectivity_access_contract_key() -> ContractKey<LocalConnectivityAccessContract> {
    ContractKey::provisional(local_connectivity_access_contract_id())
}

pub trait LocalConnectivityAccessService: Send + Sync {
    fn resolve_local_access(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<LocalConnectivityAccess, ConnectivityError>;
}

#[derive(Clone)]
pub struct LocalConnectivityAccessContract {
    inner: Arc<dyn LocalConnectivityAccessService>,
}

impl LocalConnectivityAccessContract {
    pub fn new(inner: Arc<dyn LocalConnectivityAccessService>) -> Self {
        Self { inner }
    }

    pub fn resolve_local_access(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<LocalConnectivityAccess, ConnectivityError> {
        self.inner.resolve_local_access(reachability_id)
    }
}
