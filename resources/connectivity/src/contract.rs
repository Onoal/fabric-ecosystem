use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};
use fabric_resource::ResourceId;
use fabric_resource_ingress::IngressRouteId;

use crate::{
    ConnectivityError, ConnectivityScope, PreparedReachability, Reachability, ReachabilityId,
};

const CONNECTIVITY_CONTRACT_ID: &str = "fabric.resource.connectivity";

pub fn connectivity_resource_id() -> ResourceId {
    ResourceId::new("connectivity").expect("static connectivity resource id")
}

pub fn connectivity_contract_id() -> ContractId {
    ContractId::new(CONNECTIVITY_CONTRACT_ID).expect("static connectivity contract id")
}

pub fn connectivity_contract_key() -> ContractKey<ConnectivityContract> {
    ContractKey::provisional(connectivity_contract_id())
}

pub trait ConnectivityService: Send + Sync {
    fn ensure_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<PreparedReachability, ConnectivityError>;
    fn cleanup_prepared_reachability(
        &self,
        prepared: &PreparedReachability,
    ) -> Result<(), ConnectivityError>;
    fn get_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError>;
    fn resolve_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<Reachability, ConnectivityError>;
    fn activate_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError>;
    fn deactivate_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<(), ConnectivityError>;
}

#[derive(Clone)]
pub struct ConnectivityContract {
    inner: Arc<dyn ConnectivityService>,
}

impl ConnectivityContract {
    pub fn new(inner: Arc<dyn ConnectivityService>) -> Self {
        Self { inner }
    }

    pub fn ensure_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<PreparedReachability, ConnectivityError> {
        self.inner.ensure_reachability(ingress_route_id, scope)
    }

    pub fn cleanup_prepared_reachability(
        &self,
        prepared: &PreparedReachability,
    ) -> Result<(), ConnectivityError> {
        self.inner.cleanup_prepared_reachability(prepared)
    }

    pub fn get_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError> {
        self.inner.get_reachability(reachability_id)
    }

    pub fn resolve_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<Reachability, ConnectivityError> {
        self.inner.resolve_reachability(ingress_route_id, scope)
    }

    pub fn activate_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError> {
        self.inner.activate_reachability(reachability_id)
    }

    pub fn deactivate_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<(), ConnectivityError> {
        self.inner.deactivate_reachability(reachability_id)
    }
}
