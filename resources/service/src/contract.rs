use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};

use crate::{
    MarkServiceTargetDrainingRequest, MarkServiceTargetReadyRequest, PreparedService,
    RegisterServiceTargetRequest, Service, ServiceError, ServiceHttpRequest, ServiceHttpResponse,
    ServiceHttpTargetRuntime, ServiceId, ServiceLiveEndpoint, ServiceRequirement, ServiceScope,
    ServiceTarget, WithdrawServiceTargetRequest,
};

const SERVICE_CONTRACT_ID: &str = "fabric.resource.service";

pub fn service_contract_id() -> ContractId {
    ContractId::new(SERVICE_CONTRACT_ID).expect("static service contract id")
}

pub fn service_contract_key() -> ContractKey<ServiceContract> {
    ContractKey::provisional(service_contract_id())
}

pub trait ServiceService: Send + Sync {
    fn ensure_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<PreparedService, ServiceError>;
    fn cleanup_prepared_service(&self, prepared: &PreparedService) -> Result<(), ServiceError>;
    fn resolve_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<Service, ServiceError>;
    fn get_service(&self, service_id: &ServiceId) -> Result<Service, ServiceError>;
    fn resolve_live_endpoint(
        &self,
        service_id: &ServiceId,
    ) -> Result<ServiceLiveEndpoint, ServiceError>;
    fn list_targets(&self, service_id: &ServiceId) -> Result<Vec<ServiceTarget>, ServiceError>;
    fn register_target(
        &self,
        request: RegisterServiceTargetRequest,
        runtime: ServiceHttpTargetRuntime,
    ) -> Result<(), ServiceError>;
    fn mark_target_ready(&self, request: MarkServiceTargetReadyRequest)
    -> Result<(), ServiceError>;
    fn mark_target_draining(
        &self,
        request: MarkServiceTargetDrainingRequest,
    ) -> Result<(), ServiceError>;
    fn withdraw_target(&self, request: WithdrawServiceTargetRequest) -> Result<(), ServiceError>;
    fn dispatch_http(
        &self,
        service_id: &ServiceId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError>;
}

#[derive(Clone)]
pub struct ServiceContract {
    inner: Arc<dyn ServiceService>,
}

impl ServiceContract {
    pub fn new(inner: Arc<dyn ServiceService>) -> Self {
        Self { inner }
    }

    pub fn ensure_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<PreparedService, ServiceError> {
        self.inner.ensure_service(scope, requirement)
    }

    pub fn cleanup_prepared_service(&self, prepared: &PreparedService) -> Result<(), ServiceError> {
        self.inner.cleanup_prepared_service(prepared)
    }

    pub fn resolve_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<Service, ServiceError> {
        self.inner.resolve_service(scope, requirement)
    }

    pub fn get_service(&self, service_id: &ServiceId) -> Result<Service, ServiceError> {
        self.inner.get_service(service_id)
    }

    pub fn resolve_live_endpoint(
        &self,
        service_id: &ServiceId,
    ) -> Result<ServiceLiveEndpoint, ServiceError> {
        self.inner.resolve_live_endpoint(service_id)
    }

    pub fn list_targets(&self, service_id: &ServiceId) -> Result<Vec<ServiceTarget>, ServiceError> {
        self.inner.list_targets(service_id)
    }

    pub fn register_target(
        &self,
        request: RegisterServiceTargetRequest,
        runtime: ServiceHttpTargetRuntime,
    ) -> Result<(), ServiceError> {
        self.inner.register_target(request, runtime)
    }

    pub fn mark_target_ready(
        &self,
        request: MarkServiceTargetReadyRequest,
    ) -> Result<(), ServiceError> {
        self.inner.mark_target_ready(request)
    }

    pub fn mark_target_draining(
        &self,
        request: MarkServiceTargetDrainingRequest,
    ) -> Result<(), ServiceError> {
        self.inner.mark_target_draining(request)
    }

    pub fn withdraw_target(
        &self,
        request: WithdrawServiceTargetRequest,
    ) -> Result<(), ServiceError> {
        self.inner.withdraw_target(request)
    }

    pub fn dispatch_http(
        &self,
        service_id: &ServiceId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        self.inner.dispatch_http(service_id, request)
    }
}
