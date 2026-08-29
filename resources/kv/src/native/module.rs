use std::sync::Arc;

use fabric_core::ContractRequirement;
use fabric_core::{ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime};
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::contract::{KvContract, kv_contract_key, kv_resource_id};
use crate::native::{healthy, start_noop};

use super::adapter::KvAdapter;

pub struct NativeKv {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    adapter: Arc<dyn KvAdapter>,
}

impl NativeKv {
    pub fn new(adapter: Arc<dyn KvAdapter>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.kv.native").expect("static module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            adapter,
        }
    }
}

impl ModuleRuntime for NativeKv {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![kv_contract_key().declaration()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &kv_contract_key(),
            Arc::new(KvContract::new(self.adapter.service())),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        self.adapter.initialize()
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.adapter
            .start()
            .map_err(|error| ModuleError::new(error.to_string()))?;
        if let Some(shell) = &self.resource_registry
            && let Err(error) = shell.register(self, ResourceDescriptor::new(kv_resource_id()))
        {
            self.adapter.stop();
            return Err(ModuleError::new(error.to_string()));
        }
        start_noop()
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.resource_registry {
            let _ = shell.unregister(&self.module_id);
        }
        self.adapter.stop();
    }

    fn health(&self) -> fabric_core::Health {
        healthy()
    }
}
