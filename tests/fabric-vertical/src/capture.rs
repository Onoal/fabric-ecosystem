use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource_authority::AuthorityContract;
use fabric_resource_connectivity::{ConnectivityContract, LocalConnectivityAccessContract};
use fabric_resource_database::DatabaseContract;
use fabric_resource_identity::IdentityContract;
use fabric_resource_ingress::{IngressContract, LocalHttpIngressAccessContract};
use fabric_resource_kv::KvContract;
use fabric_resource_secrets::SecretsContract;
use fabric_resource_service::ServiceContract;
use fabric_resource_worker::{WorkerContract, WorkerHttpContract};

#[derive(Clone, Default)]
pub(crate) struct CapturedContracts {
    pub(crate) identity: Arc<Mutex<Option<IdentityContract>>>,
    pub(crate) authority: Arc<Mutex<Option<AuthorityContract>>>,
    pub(crate) secrets: Arc<Mutex<Option<SecretsContract>>>,
    pub(crate) database: Arc<Mutex<Option<DatabaseContract>>>,
    pub(crate) kv: Arc<Mutex<Option<KvContract>>>,
    pub(crate) worker: Arc<Mutex<Option<WorkerContract>>>,
    pub(crate) worker_http: Arc<Mutex<Option<WorkerHttpContract>>>,
    pub(crate) service: Arc<Mutex<Option<ServiceContract>>>,
    pub(crate) ingress: Arc<Mutex<Option<IngressContract>>>,
    pub(crate) local_http_ingress_access: Arc<Mutex<Option<LocalHttpIngressAccessContract>>>,
    pub(crate) connectivity: Arc<Mutex<Option<ConnectivityContract>>>,
    pub(crate) local_connectivity_access: Arc<Mutex<Option<LocalConnectivityAccessContract>>>,
}

#[derive(Clone)]
pub(crate) struct CaptureModule {
    module_id: ModuleId,
    identity_requirement: ContractRequirement<IdentityContract>,
    authority_requirement: ContractRequirement<AuthorityContract>,
    secrets_requirement: ContractRequirement<SecretsContract>,
    database_requirement: ContractRequirement<DatabaseContract>,
    kv_requirement: ContractRequirement<KvContract>,
    worker_requirement: ContractRequirement<WorkerContract>,
    worker_http_requirement: ContractRequirement<WorkerHttpContract>,
    service_requirement: ContractRequirement<ServiceContract>,
    ingress_requirement: ContractRequirement<IngressContract>,
    local_http_ingress_access_requirement: ContractRequirement<LocalHttpIngressAccessContract>,
    connectivity_requirement: ContractRequirement<ConnectivityContract>,
    local_connectivity_access_requirement: ContractRequirement<LocalConnectivityAccessContract>,
    captured: CapturedContracts,
}

impl CaptureModule {
    pub(crate) fn new(captured: CapturedContracts) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.vertical.capture").expect("capture module id"),
            identity_requirement: ContractRequirement::provisional(
                fabric_resource_identity::identity_contract_id(),
            ),
            authority_requirement: ContractRequirement::provisional(
                fabric_resource_authority::authority_contract_id(),
            ),
            secrets_requirement: ContractRequirement::provisional(
                fabric_resource_secrets::secrets_contract_id(),
            ),
            database_requirement: ContractRequirement::provisional(
                fabric_resource_database::database_contract_id(),
            ),
            kv_requirement: ContractRequirement::provisional(fabric_resource_kv::kv_contract_id()),
            worker_requirement: ContractRequirement::provisional(
                fabric_resource_worker::worker_contract_id(),
            ),
            worker_http_requirement: ContractRequirement::provisional(
                fabric_resource_worker::worker_http_contract_id(),
            ),
            service_requirement: ContractRequirement::provisional(
                fabric_resource_service::service_contract_id(),
            ),
            ingress_requirement: ContractRequirement::provisional(
                fabric_resource_ingress::ingress_contract_id(),
            ),
            local_http_ingress_access_requirement: ContractRequirement::provisional(
                fabric_resource_ingress::local_http_ingress_access_contract_id(),
            ),
            connectivity_requirement: ContractRequirement::provisional(
                fabric_resource_connectivity::connectivity_contract_id(),
            ),
            local_connectivity_access_requirement: ContractRequirement::provisional(
                fabric_resource_connectivity::local_connectivity_access_contract_id(),
            ),
            captured,
        }
    }
}

impl ModuleRuntime for CaptureModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        Vec::new()
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.identity_requirement.id().clone(),
            self.authority_requirement.id().clone(),
            self.secrets_requirement.id().clone(),
            self.database_requirement.id().clone(),
            self.kv_requirement.id().clone(),
            self.worker_requirement.id().clone(),
            self.worker_http_requirement.id().clone(),
            self.service_requirement.id().clone(),
            self.ingress_requirement.id().clone(),
            self.local_http_ingress_access_requirement.id().clone(),
            self.connectivity_requirement.id().clone(),
            self.local_connectivity_access_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        *self.captured.identity.lock().expect("identity capture") = Some(
            (*bindings
                .resolve(&self.identity_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.authority.lock().expect("authority capture") = Some(
            (*bindings
                .resolve(&self.authority_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.secrets.lock().expect("secrets capture") = Some(
            (*bindings
                .resolve(&self.secrets_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.database.lock().expect("database capture") = Some(
            (*bindings
                .resolve(&self.database_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.kv.lock().expect("kv capture") = Some(
            (*bindings
                .resolve(&self.kv_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.worker.lock().expect("worker capture") = Some(
            (*bindings
                .resolve(&self.worker_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .captured
            .worker_http
            .lock()
            .expect("worker http capture") = Some(
            (*bindings
                .resolve(&self.worker_http_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.service.lock().expect("service capture") = Some(
            (*bindings
                .resolve(&self.service_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.captured.ingress.lock().expect("ingress capture") = Some(
            (*bindings
                .resolve(&self.ingress_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .captured
            .local_http_ingress_access
            .lock()
            .expect("local ingress access capture") = Some(
            (*bindings
                .resolve(&self.local_http_ingress_access_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .captured
            .connectivity
            .lock()
            .expect("connectivity capture") = Some(
            (*bindings
                .resolve(&self.connectivity_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .captured
            .local_connectivity_access
            .lock()
            .expect("local connectivity access capture") = Some(
            (*bindings
                .resolve(&self.local_connectivity_access_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn health(&self) -> Health {
        Health::Healthy
    }
}
