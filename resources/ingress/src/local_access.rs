use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};

use crate::{IngressError, IngressRouteId};

const LOCAL_HTTP_INGRESS_ACCESS_CONTRACT_ID: &str = "fabric.resource.ingress.local-http-access";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalHttpIngressAccess {
    pub route_id: IngressRouteId,
    pub url: String,
}

pub fn local_http_ingress_access_contract_id() -> ContractId {
    ContractId::new(LOCAL_HTTP_INGRESS_ACCESS_CONTRACT_ID)
        .expect("static local ingress access contract id")
}

pub fn local_http_ingress_access_contract_key() -> ContractKey<LocalHttpIngressAccessContract> {
    ContractKey::provisional(local_http_ingress_access_contract_id())
}

pub trait LocalHttpIngressAccessService: Send + Sync {
    fn materialize_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError>;
    fn resolve_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError>;
    fn withdraw_http_access(&self, route_id: &IngressRouteId) -> Result<(), IngressError>;
}

#[derive(Clone)]
pub struct LocalHttpIngressAccessContract {
    inner: Arc<dyn LocalHttpIngressAccessService>,
}

impl LocalHttpIngressAccessContract {
    pub fn new(inner: Arc<dyn LocalHttpIngressAccessService>) -> Self {
        Self { inner }
    }

    pub fn materialize_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError> {
        self.inner.materialize_http_access(route_id)
    }

    pub fn resolve_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError> {
        self.inner.resolve_http_access(route_id)
    }

    pub fn withdraw_http_access(&self, route_id: &IngressRouteId) -> Result<(), IngressError> {
        self.inner.withdraw_http_access(route_id)
    }
}
