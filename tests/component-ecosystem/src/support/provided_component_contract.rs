use std::sync::Arc;

use fabric_component::{Component, ComponentId, ProvidedComponentContract};
use fabric_core::{
    ContractId, ContractKey, Health, InstanceId, InstanceRuntimeContext, ModuleBindings,
    ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};

pub(crate) fn provided_component_contract_key<T>(
    contract_id: &str,
) -> ContractKey<ProvidedComponentContract<T>>
where
    T: Send + Sync + 'static,
{
    ContractKey::provisional(ContractId::new(contract_id).expect("contract id"))
}

#[derive(Clone)]
pub(crate) struct ProvidedContractModule<T>
where
    T: Send + Sync + 'static,
{
    module_id: ModuleId,
    provider_component_id: ComponentId,
    contract_key: ContractKey<ProvidedComponentContract<T>>,
    contract: Arc<T>,
    instance_id: InstanceId,
}

impl<T> ProvidedContractModule<T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn new(
        module_id: &str,
        provider_component_id: &str,
        contract_id: &str,
        contract: Arc<T>,
    ) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            provider_component_id: ComponentId::new(provider_component_id).expect("component id"),
            contract_key: provided_component_contract_key(contract_id),
            contract,
            instance_id: InstanceId::new("fabric.component.runtime.unbound")
                .expect("unbound instance id"),
        }
    }

    fn provider_component(&self) -> Component {
        Component::for_instance(self.provider_component_id.clone(), self.instance_id.clone())
    }
}

impl<T> ModuleRuntime for ProvidedContractModule<T>
where
    T: Send + Sync + 'static,
{
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![self.contract_key.id().clone()]
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        Vec::new()
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &self.contract_key,
            Arc::new(ProvidedComponentContract::new(
                self.provider_component(),
                self.module_id.clone(),
                Arc::clone(&self.contract),
            )),
        )])
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
        Ok(())
    }

    fn bind_instance_context(
        &mut self,
        context: &InstanceRuntimeContext,
    ) -> Result<(), ModuleError> {
        self.instance_id = context.instance_id().clone();
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
