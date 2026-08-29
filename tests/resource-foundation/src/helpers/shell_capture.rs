use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource_registry::ResourceRegistry;

pub(crate) type CapturedShell = Arc<Mutex<Option<Arc<ResourceRegistry>>>>;

#[derive(Clone)]
pub(crate) struct ShellCaptureModule {
    module_id: ModuleId,
    requirement: ContractRequirement<ResourceRegistry>,
    captured: CapturedShell,
}

impl ShellCaptureModule {
    pub(crate) fn new(captured: CapturedShell) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.resource.foundation.shell.capture")
                .expect("shell capture module id"),
            requirement: ContractRequirement::provisional(
                fabric_resource_registry::resource_registry_contract_id(),
            ),
            captured,
        }
    }
}

impl ModuleRuntime for ShellCaptureModule {
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
        vec![self.requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let shell = bindings
            .resolve(&self.requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.captured.lock().expect("shell capture lock") = Some(shell.clone());
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
