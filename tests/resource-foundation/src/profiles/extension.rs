use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractId, ContractKey,
    ContractRequirement, Health, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource::ResourceId;
use fabric_resource_registry::{
    ResourceConfiguration, ResourceConfigurationFacet, ResourceConfigurationKind,
    ResourceDescriptor, ResourceInspection, ResourceInspectionEntry, ResourceInspectionFacet,
    ResourceRegistry, ResourceRegistryModule,
};

use crate::helpers::shell_capture::{CapturedShell, ShellCaptureModule};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TelemetryConfig {
    shards: u16,
}

#[derive(Clone)]
struct TelemetryState {
    configured_shards: Arc<Mutex<Option<u16>>>,
}

#[derive(Clone)]
struct SyntheticProviderModule {
    module_id: ModuleId,
    provided_contracts: Vec<ContractId>,
}

impl SyntheticProviderModule {
    fn new(module_id: &str, provided_contracts: Vec<ContractId>) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            provided_contracts,
        }
    }
}

impl ModuleRuntime for SyntheticProviderModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        self.provided_contracts
            .clone()
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
        Ok(self
            .provided_contracts
            .iter()
            .map(|contract_id| {
                let key = ContractKey::<()>::provisional(contract_id.clone());
                ModuleContract::new(&key, Arc::new(()))
            })
            .collect())
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
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

#[derive(Clone)]
struct TelemetryResourceModule {
    module_id: ModuleId,
    shell_requirement: ContractRequirement<ResourceRegistry>,
    shell: Option<ResourceRegistry>,
    provider_requirement: ContractRequirement<()>,
    state: TelemetryState,
}

impl TelemetryResourceModule {
    fn new(state: TelemetryState, provider_contract: ContractKey<()>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.resource.foundation.synthetic.telemetry")
                .expect("module id"),
            shell_requirement: ContractRequirement::provisional(
                fabric_resource_registry::resource_registry_contract_id(),
            ),
            shell: None,
            provider_requirement: ContractRequirement::provisional(provider_contract.id().clone()),
            state,
        }
    }
}

impl ModuleRuntime for TelemetryResourceModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![ContractId::new("fabric.test.resource.telemetry").expect("contract id")]
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.provider_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.shell_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(self
            .provided_contract_declarations()
            .into_iter()
            .map(|declaration| {
                let key = ContractKey::<()>::provisional(declaration.id().clone());
                ModuleContract::new(&key, Arc::new(()))
            })
            .collect())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let _provider = bindings
            .resolve(&self.provider_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.shell = bindings
            .resolve_optional(&self.shell_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        if let Some(shell) = &self.shell {
            let state = self.state.clone();
            shell
                .register(
                    self,
                    ResourceDescriptor::new(ResourceId::new("telemetry").expect("resource id"))
                        .provides(
                            ContractId::new("fabric.test.resource.telemetry").expect("contract id"),
                        )
                        .requires(self.provider_requirement.id().clone())
                        .with_configuration(ResourceConfigurationFacet::typed_with(
                            ResourceConfigurationKind::new("telemetry.box")
                                .expect("configuration kind"),
                            move |config: &TelemetryConfig| {
                                *state
                                    .configured_shards
                                    .lock()
                                    .expect("telemetry state lock") = Some(config.shards);
                                Ok(())
                            },
                        ))
                        .with_inspection({
                            let state = self.state.clone();
                            ResourceInspectionFacet::new(move || {
                                let configured_shards = state
                                    .configured_shards
                                    .lock()
                                    .expect("telemetry state lock")
                                    .unwrap_or_default();
                                ResourceInspection::new(vec![
                                    ResourceInspectionEntry::public(
                                        "configured",
                                        (!configured_shards.eq(&0)).to_string(),
                                    )?,
                                    ResourceInspectionEntry::public(
                                        "shards",
                                        configured_shards.to_string(),
                                    )?,
                                ])
                            })
                        }),
                )
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.shell {
            let _ = shell.unregister(&self.module_id);
        }
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

#[test]
fn unknown_resource_participates_through_the_same_open_shell_box() {
    let capture: CapturedShell = Arc::new(Mutex::new(None));
    let telemetry_state = TelemetryState {
        configured_shards: Arc::new(Mutex::new(None)),
    };
    let provider_contract = ContractKey::<()>::provisional(
        ContractId::new("fabric.test.resource.telemetry.provider").expect("id"),
    );
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.test.resource.foundation.extension".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("resource-registry".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
            .register_module(SyntheticProviderModule::new(
                "fabric.test.resource.telemetry.provider",
                vec![provider_contract.id().clone()],
            ))
            .register_module(TelemetryResourceModule::new(
                telemetry_state.clone(),
                provider_contract,
            ))
            .register_module(ShellCaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(
            InstanceId::new("fabric.test.resource.foundation.extension").expect("instance id"),
        )
        .expect("materialize composition");
    instance.start().expect("start composition");
    let shell = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured shell");
    let resources = shell.resources();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].resource_id().as_str(), "telemetry");
    assert_eq!(
        resources[0].provided_contracts()[0].as_str(),
        "fabric.test.resource.telemetry"
    );
    assert_eq!(
        resources[0].required_contracts()[0].as_str(),
        "fabric.test.resource.telemetry.provider"
    );

    let resource_id = ResourceId::new("telemetry").expect("resource id");
    shell
        .consume_configuration(
            &resource_id,
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("telemetry.box").expect("kind"),
                TelemetryConfig { shards: 3 },
            ),
        )
        .expect("consume telemetry config");
    assert_eq!(
        *telemetry_state
            .configured_shards
            .lock()
            .expect("telemetry state lock"),
        Some(3)
    );

    let inspection = shell.inspect(&resource_id).expect("inspection");
    assert_eq!(inspection.entries()[0].key(), "configured");
    assert_eq!(inspection.entries()[0].public_value(), Some("true"));
    assert_eq!(inspection.entries()[1].key(), "shards");
    assert_eq!(inspection.entries()[1].public_value(), Some("3"));

    instance.stop();
}
