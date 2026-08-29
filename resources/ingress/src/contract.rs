use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};
use fabric_resource::ResourceId;
use fabric_resource_service::ServiceId;

use crate::{IngressError, IngressRoute, IngressRouteId, IngressRouteTarget};

const INGRESS_CONTRACT_ID: &str = "fabric.resource.ingress";

pub fn ingress_resource_id() -> ResourceId {
    ResourceId::new("ingress").expect("static ingress resource id")
}

pub fn ingress_contract_id() -> ContractId {
    ContractId::new(INGRESS_CONTRACT_ID).expect("static ingress contract id")
}

pub fn ingress_contract_key() -> ContractKey<IngressContract> {
    ContractKey::provisional(ingress_contract_id())
}

pub trait IngressService: Send + Sync {
    fn ensure_route(&self, target: &IngressRouteTarget) -> Result<IngressRoute, IngressError>;
    fn get_route(&self, route_id: &IngressRouteId) -> Result<IngressRoute, IngressError>;
    fn resolve_route(&self, service_id: &ServiceId) -> Result<IngressRoute, IngressError>;
    fn withdraw_route(&self, route_id: &IngressRouteId) -> Result<(), IngressError>;
}

#[derive(Clone)]
pub struct IngressContract {
    inner: Arc<dyn IngressService>,
}

impl IngressContract {
    pub fn new(inner: Arc<dyn IngressService>) -> Self {
        Self { inner }
    }

    pub fn ensure_route(&self, target: &IngressRouteTarget) -> Result<IngressRoute, IngressError> {
        self.inner.ensure_route(target)
    }

    pub fn get_route(&self, route_id: &IngressRouteId) -> Result<IngressRoute, IngressError> {
        self.inner.get_route(route_id)
    }

    pub fn resolve_route(&self, service_id: &ServiceId) -> Result<IngressRoute, IngressError> {
        self.inner.resolve_route(service_id)
    }

    pub fn withdraw_route(&self, route_id: &IngressRouteId) -> Result<(), IngressError> {
        self.inner.withdraw_route(route_id)
    }
}
