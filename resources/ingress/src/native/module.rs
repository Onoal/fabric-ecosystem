use std::sync::Arc;

use fabric_core::{
    ContractRequirement, Health, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};
use fabric_resource_service::{ServiceContract, service_contract_id};

use crate::{
    IngressContract, IngressService, LocalHttpIngressAccessContract, LocalHttpIngressAccessService,
    ingress_contract_key, ingress_resource_id, local_http_ingress_access_contract_key,
};

use super::adapter::IngressAdapter;

pub struct NativeIngress {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    service_requirement: ContractRequirement<ServiceContract>,
    adapter: Arc<dyn IngressAdapter>,
}

impl NativeIngress {
    pub fn new(adapter: Arc<dyn IngressAdapter>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.ingress.native")
                .expect("static ingress module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            service_requirement: ContractRequirement::provisional(service_contract_id()),
            adapter,
        }
    }
}

impl ModuleRuntime for NativeIngress {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![
            ingress_contract_key().declaration(),
            local_http_ingress_access_contract_key().declaration(),
        ]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.service_requirement.declaration().clone()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let ingress: Arc<dyn IngressService> = self.adapter.ingress_service();
        let local_http_access: Arc<dyn LocalHttpIngressAccessService> =
            self.adapter.local_http_ingress_access_service();
        Ok(vec![
            ModuleContract::new(
                &ingress_contract_key(),
                Arc::new(IngressContract::new(ingress)),
            ),
            ModuleContract::new(
                &local_http_ingress_access_contract_key(),
                Arc::new(LocalHttpIngressAccessContract::new(local_http_access)),
            ),
        ])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        let service = bindings
            .resolve(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.adapter.bind_service((*service).clone());
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        self.adapter.initialize()
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.adapter.start()?;
        if let Some(registry) = &self.resource_registry
            && let Err(error) =
                registry.register(self, ResourceDescriptor::new(ingress_resource_id()))
        {
            self.adapter.stop();
            return Err(ModuleError::new(error.to_string()));
        }
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(registry) = &self.resource_registry {
            let _ = registry.unregister(&self.module_id);
        }
        self.adapter.stop();
    }

    fn health(&self) -> Health {
        self.adapter.health()
    }
}
