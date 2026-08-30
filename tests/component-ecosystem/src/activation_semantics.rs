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
    Component, ComponentAggregateBlocker, ComponentCommunication, ComponentContract,
    ComponentEffectiveAvailability, ComponentId, ComponentParticipation, ComponentReadinessPolicy,
    ComponentReadinessRail, ComponentRegistry, ComponentRequirementKind, ComponentRequirementRail,
    ComponentRuntime, ComponentRuntimeLifecycle, ComponentRuntimeModule, InvocationRail,
    OperationId, OperationKey, OperationRail, OperationRegistrar, ParticipationState,
    ProvidedComponentContract,
};
use fabric_component_gateway::{
    Gateway, GatewayError, GatewayModule, GatewayRequest, GatewayStepId, GatewayStepPhase,
    GatewayStepRegistrar,
};
use fabric_component_namespace::NamespaceModule;
use fabric_component_publication::PublicationModule;

const REQUIREMENT_CONTRACT_ID: &str = "fabric.component.activation.requirement";
const OPERATION_ID: &str = "fabric.component.activation.echo";
const NOTES_CONTRACT_ID: &str = "fabric.component.activation.notes";

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoOutput(&'static str);

#[derive(Clone)]
struct Notes;

impl Notes {
    fn reply(&self, input: &str) -> String {
        format!("notes:{input}")
    }
}

struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    readiness: Arc<ComponentReadinessRail>,
    requirements: Arc<ComponentRequirementRail>,
    communication: Arc<ComponentCommunication>,
    invocation: Arc<InvocationRail>,
    operations: Arc<OperationRail>,
    registrar: Arc<OperationRegistrar>,
    gateway: Arc<Gateway>,
    steps: Arc<GatewayStepRegistrar>,
    resolved_requirement: ResolvedContract<ProvidedComponentContract<()>>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<Notes>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    readiness: ContractRequirement<ComponentReadinessRail>,
    requirements: ContractRequirement<ComponentRequirementRail>,
    communication: ContractRequirement<ComponentCommunication>,
    invocation: ContractRequirement<InvocationRail>,
    operations: ContractRequirement<OperationRail>,
    registrar: ContractRequirement<OperationRegistrar>,
    gateway: ContractRequirement<Gateway>,
    steps: ContractRequirement<GatewayStepRegistrar>,
    resolved_requirement: ContractRequirement<ProvidedComponentContract<()>>,
    resolved_notes: ContractRequirement<ProvidedComponentContract<Notes>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.activation.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            readiness: ContractRequirement::provisional(
                fabric_component::component_readiness_contract_id(),
            ),
            requirements: ContractRequirement::provisional(
                fabric_component::component_requirement_contract_id(),
            ),
            communication: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            operations: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            registrar: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            gateway: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            steps: ContractRequirement::provisional(
                fabric_component_gateway::gateway_step_registrar_contract_id(),
            ),
            resolved_requirement: ContractRequirement::provisional(
                requirement_contract_key().id().clone(),
            ),
            resolved_notes: ContractRequirement::provisional(notes_contract_key().id().clone()),
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
            self.readiness.id().clone(),
            self.requirements.id().clone(),
            self.communication.id().clone(),
            self.invocation.id().clone(),
            self.operations.id().clone(),
            self.registrar.id().clone(),
            self.gateway.id().clone(),
            self.steps.id().clone(),
            self.resolved_requirement.id().clone(),
            self.resolved_notes.id().clone(),
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
            readiness: bindings.resolve(&self.readiness).map_err(module_error)?,
            requirements: bindings.resolve(&self.requirements).map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication)
                .map_err(module_error)?,
            invocation: bindings.resolve(&self.invocation).map_err(module_error)?,
            operations: bindings.resolve(&self.operations).map_err(module_error)?,
            registrar: bindings.resolve(&self.registrar).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
            steps: bindings.resolve(&self.steps).map_err(module_error)?,
            resolved_requirement: bindings
                .resolve_with_provider(&self.resolved_requirement)
                .map_err(module_error)?,
            resolved_notes: bindings
                .resolve_with_provider(&self.resolved_notes)
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

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    struct ThreadWaker {
        ready: Mutex<bool>,
        wake: Condvar,
    }

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            let mut ready = self.ready.lock().expect("waker lock");
            *ready = true;
            self.wake.notify_one();
        }
    }

    let waker = Arc::new(ThreadWaker {
        ready: Mutex::new(true),
        wake: Condvar::new(),
    });
    let waker_ref = Waker::from(Arc::clone(&waker));
    let mut context = Context::from_waker(&waker_ref);
    let mut future = Pin::from(Box::new(future));
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        let mut ready = waker.ready.lock().expect("waker lock");
        while !*ready {
            ready = waker.wake.wait(ready).expect("waker wait");
        }
        *ready = false;
    }
}

fn fixture(required_components: &[&str]) -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let policy = ComponentReadinessPolicy::new(
        required_components
            .iter()
            .map(|component_id| ComponentId::new(*component_id).expect("required component id")),
    )
    .expect("policy");
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.activation").expect("composition id"))
            .register_block(
                BlockBuilder::new(BlockId::new("runtime.activation.block").expect("block id"))
                    .register_module(ComponentRuntimeModule::with_readiness_policy(policy))
                    .register_module(ProvidedContractModule::new(
                        "runtime.activation.requirement.provider",
                        "component.provider",
                        REQUIREMENT_CONTRACT_ID,
                        Arc::new(()),
                    ))
                    .register_module(ProvidedContractModule::new(
                        "runtime.activation.notes.provider",
                        "component.optional",
                        NOTES_CONTRACT_ID,
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
        .materialize(InstanceId::new("runtime.activation").expect("instance id"))
        .expect("materialize instance");
    instance.start().expect("start instance");
    let rails = capture.lock().expect("capture lock").take().expect("rails");
    (instance, rails)
}

fn component(rails: &Rails, component_id: &str) -> Component {
    Component::bind(
        ComponentId::new(component_id).expect("component id"),
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

fn operation_key() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(OPERATION_ID).expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.activation.echo.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.activation.echo.output")
            .expect("output type id"),
    )
}

fn requirement_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<()>> {
    provided_component_contract_key(REQUIREMENT_CONTRACT_ID)
}

fn notes_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<Notes>> {
    provided_component_contract_key(NOTES_CONTRACT_ID)
}

#[test]
fn registration_creates_preparing_status_and_activation_keeps_same_participation() {
    let (mut instance, rails) = fixture(&["component.required"]);
    let required = component(&rails, "component.required");
    let participation = prepare(&rails, required.clone(), Health::Healthy);
    let status = rails
        .registry
        .component(required.component_id())
        .expect("component status");

    assert_eq!(status.participation(), &participation);
    assert_eq!(status.state(), ParticipationState::Preparing);
    assert_eq!(status.health(), Health::Healthy);
    assert_eq!(
        rails
            .requirements
            .effective_availability(required.component_id())
            .expect("effective availability"),
        ComponentEffectiveAvailability::NotActive {
            component: required.clone(),
            participation: participation.clone(),
            state: ParticipationState::Preparing,
        }
    );
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Degraded
    );
    assert_eq!(
        rails.readiness.aggregate_readiness().blockers(),
        &[ComponentAggregateBlocker::RequiredComponentUnavailable {
            component_id: required.component_id().clone(),
            effective_health: fabric_component::ComponentEffectiveHealth::Unavailable(
                ComponentEffectiveAvailability::NotActive {
                    component: required.clone(),
                    participation: participation.clone(),
                    state: ParticipationState::Preparing,
                },
            ),
        }]
    );

    let activated = rails.registry.activate(&participation).expect("activate");
    assert_eq!(activated.participation(), &participation);
    assert_eq!(activated.state(), ParticipationState::Active);
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );
    instance.stop();
}

#[test]
fn staged_operation_is_invisible_until_activation_and_runtime_shape_freezes_afterward() {
    let (mut instance, rails) = fixture(&[]);
    let owner = component(&rails, "component.operation");
    let participation = prepare(&rails, owner.clone(), Health::Healthy);
    let events = Arc::new(Mutex::new(Vec::new()));
    let dispatch_events = Arc::clone(&events);
    rails
        .registrar
        .register(
            participation.clone(),
            operation_key(),
            move |input: EchoInput| {
                let events = Arc::clone(&dispatch_events);
                async move {
                    events.lock().expect("events").push(input.0);
                    Ok(EchoOutput(input.0))
                }
            },
        )
        .expect("stage operation");

    assert_eq!(
        block_on(rails.operations.invoke_with_context(
            rails.invocation.begin_external().expect("gateway root"),
            &operation_key(),
            EchoInput("before"),
        )),
        Err(fabric_component::ComponentError::ComponentUnavailable(
            owner.component_id().clone()
        ))
    );
    assert!(events.lock().expect("events").is_empty());

    rails.registry.activate(&participation).expect("activate");
    assert_eq!(
        block_on(rails.operations.invoke_with_context(
            rails.invocation.begin_external().expect("gateway root"),
            &operation_key(),
            EchoInput("after"),
        )),
        Ok(EchoOutput("after"))
    );
    assert_eq!(*events.lock().expect("events"), vec!["after"]);
    assert!(matches!(
        rails.registrar.register(
            participation,
            OperationKey::new(
                OperationId::new("fabric.component.activation.second").expect("operation id"),
                fabric_component::OperationTypeId::new("fabric.test.activation.second.input")
                    .expect("input type id"),
                fabric_component::OperationTypeId::new("fabric.test.activation.second.output")
                    .expect("output type id"),
            ),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        ),
        Err(fabric_component::ComponentError::OperationOwnerNotPreparing { .. })
    ));
    instance.stop();
}

#[test]
fn staged_gateway_guard_fails_closed_until_activation_and_active_owner_cannot_add_more_steps() {
    let (mut instance, rails) = fixture(&[]);
    let operation_owner = component(&rails, "component.operation");
    let guard_owner = component(&rails, "component.guard");
    let operation_participation = prepare(&rails, operation_owner.clone(), Health::Healthy);
    let guard_participation = prepare(&rails, guard_owner.clone(), Health::Healthy);
    let events = Arc::new(Mutex::new(Vec::new()));
    let operation_events = Arc::clone(&events);
    rails
        .registrar
        .register_with_context(
            operation_participation.clone(),
            operation_key(),
            move |_, input| {
                let events = Arc::clone(&operation_events);
                async move {
                    events.lock().expect("events").push(input.0);
                    Ok(EchoOutput(input.0))
                }
            },
        )
        .expect("stage operation");
    let guard_events = Arc::clone(&events);
    rails
        .steps
        .register(
            guard_participation.clone(),
            GatewayStepId::new("step.guard").expect("step id"),
            GatewayStepPhase::Guard,
            |_| true,
            move |_| {
                guard_events.lock().expect("events").push("guard");
                Ok(())
            },
        )
        .expect("stage guard");
    rails
        .registry
        .activate(&operation_participation)
        .expect("activate operation");

    assert_eq!(
        block_on(
            rails
                .gateway
                .invoke(GatewayRequest::new(operation_key(), EchoInput("blocked"),))
        ),
        Err(GatewayError::Component(
            fabric_component::ComponentError::ComponentUnavailable(
                guard_owner.component_id().clone(),
            ),
        ))
    );
    assert!(events.lock().expect("events").is_empty());

    rails
        .registry
        .activate(&guard_participation)
        .expect("activate guard");
    assert_eq!(
        block_on(
            rails
                .gateway
                .invoke(GatewayRequest::new(operation_key(), EchoInput("ok"),))
        )
        .expect("gateway")
        .into_output(),
        EchoOutput("ok")
    );
    assert_eq!(*events.lock().expect("events"), vec!["guard", "ok"]);
    assert!(matches!(
        rails.steps.register(
            guard_participation,
            GatewayStepId::new("step.extra").expect("step id"),
            GatewayStepPhase::Guard,
            |_| true,
            |_| Ok(()),
        ),
        Err(GatewayError::GatewayStepOwnerNotPreparing(_, _))
    ));
    instance.stop();
}

#[test]
fn required_preparing_dependency_blocks_consumer_until_provider_activation_but_optional_call_still_fails()
 {
    let (mut instance, rails) = fixture(&["component.consumer"]);
    let consumer = component(&rails, "component.consumer");
    let provider = component(&rails, "component.provider");
    let consumer_participation = prepare(&rails, consumer.clone(), Health::Healthy);
    let provider_participation = prepare(&rails, provider.clone(), Health::Healthy);
    rails
        .requirements
        .register_resolved(
            consumer.clone(),
            requirement_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails.resolved_requirement.clone(),
        )
        .expect("register required dependency");
    assert_eq!(
        rails
            .requirements
            .requirements(consumer.component_id())
            .first()
            .expect("registered requirement")
            .provider(),
        &provider
    );
    rails
        .registry
        .activate(&consumer_participation)
        .expect("activate consumer");

    let readiness = rails.readiness.aggregate_readiness();
    assert_eq!(
        readiness.status().lifecycle(),
        ComponentRuntimeLifecycle::Degraded
    );
    assert_eq!(readiness.status().health(), Health::Unavailable);
    match rails
        .requirements
        .effective_availability(consumer.component_id())
        .expect("effective availability")
    {
        ComponentEffectiveAvailability::RequiredDependencyUnavailable(blocker) => {
            assert_eq!(blocker.provider(), &provider);
            assert_eq!(
                blocker.provider_availability(),
                &ComponentEffectiveAvailability::NotActive {
                    component: provider.clone(),
                    participation: provider_participation.clone(),
                    state: ParticipationState::Preparing,
                }
            );
        }
        other => panic!("expected required dependency blocker, got {other:?}"),
    }

    rails
        .registry
        .activate(&provider_participation)
        .expect("activate provider");
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );

    let optional_provider = component(&rails, "component.optional");
    let optional_participation = prepare(&rails, optional_provider.clone(), Health::Healthy);
    rails
        .requirements
        .register_resolved(
            consumer.clone(),
            notes_contract_key().id().clone(),
            ComponentRequirementKind::Optional,
            rails.resolved_notes.clone(),
        )
        .expect("register optional dependency");
    let notes: ComponentContract<Notes> = rails
        .communication
        .bind_resolved(
            consumer.clone(),
            notes_contract_key().id().clone(),
            ComponentRequirementKind::Optional,
            rails.resolved_notes.clone(),
        )
        .expect("bind notes");
    assert_eq!(notes.provider(), &optional_provider);

    assert_eq!(
        rails
            .requirements
            .effective_availability(consumer.component_id())
            .expect("consumer availability"),
        ComponentEffectiveAvailability::Available
    );
    assert_eq!(
        notes.call(&consumer_participation, |notes| notes.reply("blocked")),
        Err(fabric_component::ComponentError::ComponentUnavailable(
            optional_provider.component_id().clone()
        ))
    );
    assert_eq!(
        rails
            .requirements
            .effective_availability(optional_provider.component_id())
            .expect("provider availability"),
        ComponentEffectiveAvailability::NotActive {
            component: optional_provider,
            participation: optional_participation,
            state: ParticipationState::Preparing,
        }
    );
    instance.stop();
}
