use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    ResolvedContract,
};

use crate::support::provided_component_contract::{
    ProvidedContractModule, provided_component_contract_key,
};
use fabric_component::{
    Component, ComponentCommunication, ComponentContract, ComponentError, ComponentId,
    ComponentParticipation, ComponentRegistry, ComponentRequirementKind, ComponentRequirementRail,
    ComponentRuntime, ComponentRuntimeModule, InvocationContext, InvocationRail, OperationId,
    OperationKey, OperationRegistrar, ProvidedComponentContract,
};
use fabric_component_gateway::{
    Gateway, GatewayError, GatewayModule, GatewayRequest, GatewayStepId, GatewayStepPhase,
    GatewayStepRegistrar,
};
use fabric_component_namespace::NamespaceModule;
use fabric_component_publication::PublicationModule;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Echo(String);

#[derive(Clone)]
struct Notes;

impl Notes {
    fn reply(&self, context: &InvocationContext, input: &str) -> String {
        format!("{}:{input}", context.invocation_id())
    }
}

struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    invocation: Arc<InvocationRail>,
    communication: Arc<ComponentCommunication>,
    registrar: Arc<OperationRegistrar>,
    operations: Arc<fabric_component::OperationRail>,
    gateway: Arc<Gateway>,
    requirements: Arc<ComponentRequirementRail>,
    steps: Arc<GatewayStepRegistrar>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<Notes>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    invocation: ContractRequirement<InvocationRail>,
    communication: ContractRequirement<ComponentCommunication>,
    registrar: ContractRequirement<OperationRegistrar>,
    operations: ContractRequirement<fabric_component::OperationRail>,
    gateway: ContractRequirement<Gateway>,
    requirements: ContractRequirement<ComponentRequirementRail>,
    steps: ContractRequirement<GatewayStepRegistrar>,
    notes: ContractRequirement<ProvidedComponentContract<Notes>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.generation.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            communication: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            registrar: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            operations: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            gateway: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            requirements: ContractRequirement::provisional(
                fabric_component::component_requirement_contract_id(),
            ),
            steps: ContractRequirement::provisional(
                fabric_component_gateway::gateway_step_registrar_contract_id(),
            ),
            notes: ContractRequirement::provisional(notes_contract_key().id().clone()),
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
            self.runtime.id().clone(),
            self.registry.id().clone(),
            self.invocation.id().clone(),
            self.communication.id().clone(),
            self.registrar.id().clone(),
            self.operations.id().clone(),
            self.gateway.id().clone(),
            self.requirements.id().clone(),
            self.steps.id().clone(),
            self.notes.id().clone(),
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
            runtime: bindings.resolve(&self.runtime).map_err(module_error)?,
            registry: bindings.resolve(&self.registry).map_err(module_error)?,
            invocation: bindings.resolve(&self.invocation).map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication)
                .map_err(module_error)?,
            registrar: bindings.resolve(&self.registrar).map_err(module_error)?,
            operations: bindings.resolve(&self.operations).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
            requirements: bindings.resolve(&self.requirements).map_err(module_error)?,
            steps: bindings.resolve(&self.steps).map_err(module_error)?,
            resolved_notes: bindings
                .resolve_with_provider(&self.notes)
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

fn fixture() -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let composition = CompositionBuilder::new(
        CompositionId::new("runtime.runtime.generation").expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("runtime.runtime.generation.block").expect("block id"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(ProvidedContractModule::new(
                "runtime.runtime.generation.notes.provider",
                "component.provider",
                "runtime.generation.notes",
                Arc::new(Notes),
            ))
            .register_module(NamespaceModule::new())
            .register_module(PublicationModule::new())
            .register_module(GatewayModule::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = composition
        .materialize(InstanceId::new("runtime.runtime.generation").expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start");
    let rails = capture.lock().expect("capture lock").take().expect("rails");
    (instance, rails)
}

fn component(rails: &Rails, id: &str) -> Component {
    Component::bind(
        ComponentId::new(id).expect("component id"),
        rails.runtime.as_ref(),
    )
}

fn prepare(rails: &Rails, component: Component, health: Health) -> ComponentParticipation {
    rails
        .registry
        .register(component, health)
        .expect("prepare")
        .participation()
        .clone()
}

fn participate(rails: &Rails, component: Component, health: Health) -> ComponentParticipation {
    let participation = prepare(rails, component, health);
    rails
        .registry
        .activate(&participation)
        .expect("activate participation");
    participation
}

fn echo_operation() -> OperationKey<Echo, Echo> {
    OperationKey::new(
        OperationId::new("generation.echo").expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.generation.echo.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.generation.echo.output")
            .expect("output type id"),
    )
}

fn notes_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<Notes>> {
    provided_component_contract_key("runtime.generation.notes")
}

fn bind_notes(rails: &Rails, consumer: Component, provider: Component) -> ComponentContract<Notes> {
    let notes = rails
        .communication
        .bind_resolved(
            consumer.clone(),
            notes_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails.resolved_notes.clone(),
        )
        .expect("bind notes");
    assert_eq!(
        rails
            .requirements
            .requirements(consumer.component_id())
            .len(),
        1
    );
    assert_eq!(notes.provider(), &provider);
    notes
}

struct Parker {
    ready: Mutex<bool>,
    cvar: Condvar,
}

impl Wake for Parker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let mut ready = self.ready.lock().expect("parker lock");
        *ready = true;
        self.cvar.notify_one();
    }
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let parker = Arc::new(Parker {
        ready: Mutex::new(false),
        cvar: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&parker));
    let mut context = Context::from_waker(&waker);
    let mut future = Pin::from(Box::new(future));

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                let mut ready = parker.ready.lock().expect("parker lock");
                while !*ready {
                    ready = parker.cvar.wait(ready).expect("parker wait");
                }
                *ready = false;
            }
        }
    }
}

#[test]
fn fresh_runtimes_for_same_instance_are_generation_distinct() {
    let (mut runtime_one, rails_one) = fixture();
    let (mut runtime_two, rails_two) = fixture();

    let status_one = rails_one.runtime.current_status();
    let status_two = rails_two.runtime.current_status();
    assert_eq!(status_one.instance_id(), status_two.instance_id());
    assert_eq!(status_one.instance_id(), status_two.instance_id(),);
    assert_ne!(status_one.generation(), status_two.generation());

    let gateway_one = rails_one
        .invocation
        .begin_external()
        .expect("runtime one root");
    let gateway_two = rails_two
        .invocation
        .begin_external()
        .expect("runtime two root");
    assert_eq!(gateway_one.invocation_id(), gateway_two.invocation_id());
    assert_eq!(gateway_one.instance_id(), gateway_two.instance_id());
    assert_ne!(gateway_one.generation(), gateway_two.generation());

    runtime_one.stop();
    runtime_two.stop();
}

#[test]
fn old_runtime_participation_authority_is_rejected_by_fresh_runtime_rails() {
    let (mut runtime_one, rails_one) = fixture();
    let (mut runtime_two, rails_two) = fixture();

    let owner_one = component(&rails_one, "component.runtime.owner");
    let owner_two = component(&rails_two, "component.runtime.owner");
    let stale = prepare(&rails_one, owner_one, Health::Healthy);
    let current = prepare(&rails_two, owner_two, Health::Healthy);

    assert_eq!(stale.participation_id(), current.participation_id());
    assert_ne!(stale.generation(), current.generation());
    assert_ne!(stale, current);

    assert_eq!(
        rails_two.registry.update_health(&stale, Health::Degraded),
        Err(ComponentError::StaleComponentParticipation(stale.clone()))
    );
    assert_eq!(
        rails_two.registry.unregister(&stale),
        Err(ComponentError::StaleComponentParticipation(stale.clone()))
    );
    assert_eq!(
        rails_two.registry.activate(&stale),
        Err(ComponentError::StaleComponentParticipation(stale.clone()))
    );
    assert_eq!(
        rails_two.invocation.begin_component(stale.clone()),
        Err(ComponentError::InvocationOriginNotParticipating(
            stale.component().component_id().clone(),
        ))
    );
    assert_eq!(
        rails_two
            .registrar
            .register(stale.clone(), echo_operation(), |input: Echo| async move {
                Ok(input)
            },),
        Err(ComponentError::OperationOwnerNotParticipating {
            operation_id: echo_operation().id().clone(),
            component_id: stale.component().component_id().clone(),
        })
    );
    assert_eq!(
        rails_two.steps.register(
            stale.clone(),
            GatewayStepId::new("generation.step").expect("step id"),
            GatewayStepPhase::Guard,
            |_| true,
            |_| Ok(()),
        ),
        Err(GatewayError::GatewayStepOwnerNotParticipating(
            GatewayStepId::new("generation.step").expect("step id"),
            stale.component().component_id().clone(),
        ))
    );
    assert_eq!(
        rails_two
            .registry
            .component(current.component().component_id())
            .expect("current status")
            .participation(),
        &current
    );

    runtime_one.stop();
    runtime_two.stop();
}

#[test]
fn old_runtime_context_is_rejected_by_operation_gateway_and_contract_admission() {
    let (mut runtime_one, rails_one) = fixture();
    let (mut runtime_two, rails_two) = fixture();

    let stale_context = rails_one
        .invocation
        .begin_external()
        .expect("stale context");
    let provider = component(&rails_two, "component.provider");
    let caller = component(&rails_two, "component.caller");
    participate(&rails_two, provider.clone(), Health::Healthy);
    let caller_participation = prepare(&rails_two, caller.clone(), Health::Healthy);
    let notes = bind_notes(&rails_two, caller.clone(), provider);
    rails_two
        .registrar
        .register(
            caller_participation.clone(),
            echo_operation(),
            |input: Echo| async move { Ok(input) },
        )
        .expect("register echo");
    rails_two
        .registry
        .activate(&caller_participation)
        .expect("activate caller");

    assert_eq!(
        block_on(rails_two.operations.invoke_with_context(
            stale_context.clone(),
            &echo_operation(),
            Echo("operation".into()),
        )),
        Err(ComponentError::InvocationContextGenerationMismatch {
            context_generation: stale_context.generation(),
            generation: rails_two
                .runtime
                .current_status()
                .generation()
                .expect("current generation"),
        })
    );
    assert_eq!(
        block_on(rails_two.gateway.invoke_with_context(
            stale_context.clone(),
            GatewayRequest::new(echo_operation(), Echo("gateway".into())),
        )),
        Err(GatewayError::Component(
            ComponentError::InvocationContextGenerationMismatch {
                context_generation: stale_context.generation(),
                generation: rails_two
                    .runtime
                    .current_status()
                    .generation()
                    .expect("current generation"),
            },
        ))
    );
    assert_eq!(
        notes.call_with_context(&caller_participation, &stale_context, |context, notes| {
            notes.reply(context, "contract")
        }),
        Err(ComponentError::InvocationContextGenerationMismatch {
            context_generation: stale_context.generation(),
            generation: rails_two
                .runtime
                .current_status()
                .generation()
                .expect("current generation"),
        })
    );

    runtime_one.stop();
    runtime_two.stop();
}

#[test]
fn same_runtime_stale_participation_fencing_still_holds_after_rejoin() {
    let (mut instance, rails) = fixture();
    let owner = component(&rails, "component.rejoin");
    let first = participate(&rails, owner.clone(), Health::Healthy);
    rails.registry.unregister(&first).expect("unregister first");
    let second = participate(&rails, owner, Health::Healthy);

    assert_eq!(first.generation(), second.generation());
    assert_ne!(first.participation_id(), second.participation_id());
    assert_eq!(
        rails.registry.update_health(&first, Health::Degraded),
        Err(ComponentError::StaleComponentParticipation(first.clone()))
    );
    assert_eq!(
        rails.invocation.begin_component(first),
        Err(ComponentError::InvocationOriginNotParticipating(
            ComponentId::new("component.rejoin").expect("component id"),
        ))
    );
    assert!(rails.invocation.begin_component(second).is_ok());

    instance.stop();
}
