use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentCommunication, ComponentContract, ComponentError, ComponentId,
    ComponentParticipation, ComponentRegistry, ComponentRequirementKind, ComponentRuntime,
    ComponentRuntimeModule, InvocationContext, InvocationRail, ProvidedComponentContract,
};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractId, ContractKey,
    ContractRequirement, Health, Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError,
    ModuleId, ModuleRuntime, ResolvedContract,
};

const INSTANCE_ID: &str = "fabric.fabric.vertical.cross-api.generation";
const OTHER_INSTANCE_ID: &str = "fabric.fabric.vertical.cross-api.generation.other";
const NOTES_CONTRACT_ID: &str = "fabric.test.cross-api.notes";
const PROVIDER_COMPONENT_ID: &str = "component.provider";
const CALLER_COMPONENT_ID: &str = "component.caller";
const PROVIDER_MODULE_ID: &str = "fabric.test.cross-api.notes.provider";

#[derive(Clone)]
struct Notes;

impl Notes {
    fn reply(&self, context: &InvocationContext) -> String {
        format!("{}:notes", context.invocation_id())
    }
}

fn notes_contract_key() -> ContractKey<ProvidedComponentContract<Notes>> {
    ContractKey::provisional(ContractId::new(NOTES_CONTRACT_ID).expect("contract id"))
}

#[derive(Clone)]
struct NotesProviderModule {
    module_id: ModuleId,
    provider_component_id: ComponentId,
    instance_id: InstanceId,
}

impl NotesProviderModule {
    fn new() -> Self {
        Self {
            module_id: ModuleId::new(PROVIDER_MODULE_ID).expect("module id"),
            provider_component_id: ComponentId::new(PROVIDER_COMPONENT_ID).expect("component id"),
            instance_id: InstanceId::new("fabric.component.runtime.unbound")
                .expect("unbound instance id"),
        }
    }

    fn provider_component(&self) -> Component {
        Component::for_instance(self.provider_component_id.clone(), self.instance_id.clone())
    }
}

impl ModuleRuntime for NotesProviderModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![notes_contract_key().id().clone()]
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
            &notes_contract_key(),
            Arc::new(ProvidedComponentContract::new(
                self.provider_component(),
                self.module_id.clone(),
                Arc::new(Notes),
            )),
        )])
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
        Ok(())
    }

    fn bind_instance_context(
        &mut self,
        context: &fabric_core::InstanceRuntimeContext,
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

#[derive(Clone)]
struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    communication: Arc<ComponentCommunication>,
    invocation: Arc<InvocationRail>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<Notes>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    communication_requirement: ContractRequirement<ComponentCommunication>,
    invocation_requirement: ContractRequirement<InvocationRail>,
    notes_requirement: ContractRequirement<ProvidedComponentContract<Notes>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.cross-api.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            communication_requirement: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            invocation_requirement: ContractRequirement::provisional(
                fabric_component::invocation_contract_id(),
            ),
            notes_requirement: ContractRequirement::provisional(notes_contract_key().id().clone()),
            capture,
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
            self.runtime_requirement.id().clone(),
            self.registry_requirement.id().clone(),
            self.communication_requirement.id().clone(),
            self.invocation_requirement.id().clone(),
            self.notes_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        *self.capture.lock().expect("capture lock") = Some(Rails {
            runtime: bindings
                .resolve(&self.runtime_requirement)
                .map_err(module_error)?,
            registry: bindings
                .resolve(&self.registry_requirement)
                .map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication_requirement)
                .map_err(module_error)?,
            invocation: bindings
                .resolve(&self.invocation_requirement)
                .map_err(module_error)?,
            resolved_notes: bindings
                .resolve_with_provider(&self.notes_requirement)
                .map_err(module_error)?,
        });
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

fn module_error(error: impl std::fmt::Display) -> ModuleError {
    ModuleError::new(error.to_string())
}

fn component(runtime: &ComponentRuntime, id: &str) -> Component {
    Component::bind(ComponentId::new(id).expect("component id"), runtime)
}

fn participate(
    registry: &ComponentRegistry,
    component: Component,
) -> Result<ComponentParticipation, ComponentError> {
    let status = registry.register(component, Health::Healthy)?;
    let status = registry.activate(status.participation())?;
    Ok(status.participation().clone())
}

fn build_notes_handle(rails: &Rails, consumer: Component) -> ComponentContract<Notes> {
    rails
        .communication
        .bind_resolved(
            consumer,
            notes_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails.resolved_notes.clone(),
        )
        .expect("bind notes contract")
}

struct CompositionFixture {
    composition: fabric_core::Composition,
    capture: Capture,
}

impl CompositionFixture {
    fn start(&self, instance_id: &str) -> (Instance, Rails) {
        *self.capture.lock().expect("capture lock") = None;
        let mut instance = self
            .composition
            .materialize(InstanceId::new(instance_id).expect("instance id"))
            .expect("materialize composition");
        instance.start().expect("start composition");
        let rails = self
            .capture
            .lock()
            .expect("capture lock")
            .clone()
            .expect("captured rails");
        (instance, rails)
    }
}

fn fixture() -> CompositionFixture {
    let capture = Arc::new(Mutex::new(None));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.fabric.vertical.cross-api.generation").expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("components").expect("block id"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(NotesProviderModule::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    CompositionFixture {
        composition,
        capture,
    }
}

#[test]
fn same_composition_same_instance_id_rematerializes_with_a_fresh_generation() {
    let fixture = fixture();

    let (mut first_instance, first_rails) = fixture.start(INSTANCE_ID);
    let first_generation = first_rails
        .runtime
        .current_status()
        .generation()
        .expect("first generation");
    first_instance.stop();

    let (mut second_instance, second_rails) = fixture.start(INSTANCE_ID);
    let second_generation = second_rails
        .runtime
        .current_status()
        .generation()
        .expect("second generation");

    assert_eq!(first_instance.instance_id(), second_instance.instance_id());
    assert_eq!(
        first_instance.instance_id(),
        &InstanceId::new(INSTANCE_ID).expect("instance id")
    );
    assert_ne!(first_generation, second_generation);

    second_instance.stop();
}

#[test]
fn same_composition_materializes_distinct_instance_ids_without_rebuild() {
    let fixture = fixture();

    let (mut first_instance, first_rails) = fixture.start(INSTANCE_ID);
    let first_provider = component(first_rails.runtime.as_ref(), PROVIDER_COMPONENT_ID);
    let first_caller = component(first_rails.runtime.as_ref(), CALLER_COMPONENT_ID);
    let first_provider_participation =
        participate(first_rails.registry.as_ref(), first_provider.clone())
            .expect("first provider participation");
    let first_caller_participation =
        participate(first_rails.registry.as_ref(), first_caller.clone())
            .expect("first caller participation");
    let first_notes = build_notes_handle(&first_rails, first_caller.clone());
    let first_context = first_rails
        .invocation
        .begin_component(first_caller_participation.clone())
        .expect("first invocation context");

    let (mut second_instance, second_rails) = fixture.start(OTHER_INSTANCE_ID);
    let second_provider = component(second_rails.runtime.as_ref(), PROVIDER_COMPONENT_ID);
    let second_caller = component(second_rails.runtime.as_ref(), CALLER_COMPONENT_ID);
    let _second_provider_participation =
        participate(second_rails.registry.as_ref(), second_provider.clone())
            .expect("second provider participation");
    let second_caller_participation =
        participate(second_rails.registry.as_ref(), second_caller.clone())
            .expect("second caller participation");
    let second_notes = build_notes_handle(&second_rails, second_caller.clone());

    assert_ne!(first_instance.instance_id(), second_instance.instance_id());
    assert_ne!(first_instance.generation(), second_instance.generation());
    assert_eq!(
        first_notes
            .call_with_context(
                &first_caller_participation,
                &first_context,
                |context, notes| { notes.reply(context) }
            )
            .expect("first call"),
        format!("{}:notes", first_context.invocation_id())
    );
    assert_eq!(
        second_notes.call_with_context(
            &second_caller_participation,
            &first_context,
            |context, notes| notes.reply(context),
        ),
        Err(ComponentError::InvocationContextInstanceMismatch {
            context_instance_id: first_context.instance_id().clone(),
            instance_id: second_instance.instance_id().clone(),
        })
    );
    assert_eq!(
        second_rails
            .registry
            .update_health(&first_provider_participation, Health::Healthy),
        Err(ComponentError::ComponentRegistryInstanceMismatch {
            component_id: first_provider_participation
                .component()
                .component_id()
                .clone(),
            component_instance_id: first_provider_participation
                .component()
                .instance_id()
                .clone(),
            instance_id: second_instance.instance_id().clone(),
        })
    );

    second_instance.stop();
    first_instance.stop();
}

#[test]
fn stale_component_authority_is_rejected_after_same_id_rematerialization() {
    let fixture = fixture();

    let (mut first_instance, first_rails) = fixture.start(INSTANCE_ID);
    let provider_component = component(first_rails.runtime.as_ref(), PROVIDER_COMPONENT_ID);
    let caller_component = component(first_rails.runtime.as_ref(), CALLER_COMPONENT_ID);
    let provider_first = participate(first_rails.registry.as_ref(), provider_component)
        .expect("first provider participation");
    let caller_first = participate(first_rails.registry.as_ref(), caller_component.clone())
        .expect("first caller participation");
    let first_notes = build_notes_handle(&first_rails, caller_component);
    let stale_context = first_rails
        .invocation
        .begin_component(caller_first.clone())
        .expect("first invocation context");
    let first_generation = stale_context.generation();
    assert_eq!(
        first_notes
            .call_with_context(&caller_first, &stale_context, |context, notes| notes
                .reply(context))
            .expect("first call"),
        format!("{}:notes", stale_context.invocation_id())
    );
    first_instance.stop();

    let (mut second_instance, second_rails) = fixture.start(INSTANCE_ID);
    let provider_component = component(second_rails.runtime.as_ref(), PROVIDER_COMPONENT_ID);
    let caller_component = component(second_rails.runtime.as_ref(), CALLER_COMPONENT_ID);
    let _provider_second = participate(second_rails.registry.as_ref(), provider_component)
        .expect("second provider participation");
    let caller_second = participate(second_rails.registry.as_ref(), caller_component.clone())
        .expect("second caller participation");
    let second_notes = build_notes_handle(&second_rails, caller_component);
    let current_context = second_rails
        .invocation
        .begin_component(caller_second.clone())
        .expect("second invocation context");
    let second_generation = current_context.generation();

    assert_ne!(first_generation, second_generation);
    assert_eq!(
        second_rails
            .registry
            .update_health(&caller_first, Health::Degraded),
        Err(ComponentError::StaleComponentParticipation(
            caller_first.clone()
        ))
    );
    assert_eq!(
        second_rails
            .registry
            .update_health(&provider_first, Health::Degraded),
        Err(ComponentError::StaleComponentParticipation(
            provider_first.clone()
        ))
    );
    assert_eq!(
        second_notes.call_with_context(&caller_first, &stale_context, |context, notes| {
            notes.reply(context)
        }),
        Err(ComponentError::InvocationContextGenerationMismatch {
            context_generation: first_generation,
            generation: second_generation,
        })
    );

    second_instance.stop();
}
