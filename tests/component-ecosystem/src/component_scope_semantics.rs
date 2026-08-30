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
    ComponentMaterializer, ComponentReadinessPolicy, ComponentReadinessRail, ComponentRegistry,
    ComponentRequirementKind, ComponentRequirementRail, ComponentRuntime,
    ComponentRuntimeDefinition, ComponentRuntimeLifecycle, ComponentRuntimeModule,
    ComponentRuntimeScope, ComponentScope, InvocationContext, InvocationOrigin, InvocationRail,
    OperationId, OperationKey, OperationRail, ProvidedComponentContract,
};
use fabric_component_gateway::{Gateway, GatewayModule, GatewayRequest};
use fabric_component_namespace::NamespaceModule;
use fabric_component_publication::PublicationModule;

const NOTES_CONTRACT_ID: &str = "fabric.component.component.scope.notes";

#[derive(Clone, Debug, PartialEq, Eq)]
struct ContextInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct ContextOutput {
    context: InvocationContext,
    reply: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NestedContextOutput {
    outer: InvocationContext,
    inner: InvocationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NotesReply {
    context: InvocationContext,
    reply: String,
}

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
    materializer: Arc<ComponentMaterializer>,
    communication: Arc<ComponentCommunication>,
    readiness: Arc<ComponentReadinessRail>,
    gateway: Arc<Gateway>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<Notes>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    materializer: ContractRequirement<ComponentMaterializer>,
    communication: ContractRequirement<ComponentCommunication>,
    invocation: ContractRequirement<InvocationRail>,
    operations: ContractRequirement<OperationRail>,
    readiness: ContractRequirement<ComponentReadinessRail>,
    requirements: ContractRequirement<ComponentRequirementRail>,
    gateway: ContractRequirement<Gateway>,
    notes: ContractRequirement<ProvidedComponentContract<Notes>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.component.scope.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            materializer: ContractRequirement::provisional(
                fabric_component::component_materializer_contract_id(),
            ),
            communication: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            operations: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            readiness: ContractRequirement::provisional(
                fabric_component::component_readiness_contract_id(),
            ),
            requirements: ContractRequirement::provisional(
                fabric_component::component_requirement_contract_id(),
            ),
            gateway: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
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
            self.materializer.id().clone(),
            self.communication.id().clone(),
            self.invocation.id().clone(),
            self.operations.id().clone(),
            self.readiness.id().clone(),
            self.requirements.id().clone(),
            self.gateway.id().clone(),
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
            materializer: bindings.resolve(&self.materializer).map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication)
                .map_err(module_error)?,
            readiness: bindings.resolve(&self.readiness).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
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

fn definition(
    component_id: &str,
    prepare: impl Fn(&ComponentRuntimeScope) -> Result<Health, ComponentError> + Send + Sync + 'static,
) -> ComponentRuntimeDefinition {
    ComponentRuntimeDefinition::new(
        ComponentId::new(component_id).expect("component id"),
        prepare,
    )
}

fn fixture(
    instance_id: &str,
    required_components: &[&str],
    runtime_definitions: impl IntoIterator<Item = ComponentRuntimeDefinition>,
) -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let policy = ComponentReadinessPolicy::new(
        required_components
            .iter()
            .map(|component_id| ComponentId::new(*component_id).expect("required component id")),
    )
    .expect("policy");
    let composition = CompositionBuilder::new(
        CompositionId::new("runtime.component.scope").expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("runtime.component.scope.block").expect("block id"))
            .register_module(
                ComponentRuntimeModule::with_configuration(policy, runtime_definitions)
                    .expect("component runtime"),
            )
            .register_module(ProvidedContractModule::new(
                "runtime.component.scope.notes.provider",
                "component.provider",
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
        .materialize(InstanceId::new(instance_id).expect("instance id"))
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

fn notes_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<Notes>> {
    provided_component_contract_key(NOTES_CONTRACT_ID)
}

fn bind_notes(rails: &Rails, consumer: Component, provider: Component) -> ComponentContract<Notes> {
    let notes = rails
        .communication
        .bind_resolved(
            consumer,
            notes_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails.resolved_notes.clone(),
        )
        .expect("bind notes");
    assert_eq!(notes.provider(), &provider);
    notes
}

fn operation_a() -> OperationKey<ContextInput, NestedContextOutput> {
    OperationKey::new(
        OperationId::new("component.scope.a").expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.component_scope.a.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.component_scope.a.output")
            .expect("output type id"),
    )
}

fn operation_b() -> OperationKey<ContextInput, ContextOutput> {
    OperationKey::new(
        OperationId::new("component.scope.b").expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.component_scope.b.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.component_scope.b.output")
            .expect("output type id"),
    )
}

#[test]
fn preparing_scope_can_be_captured_and_becomes_usable_after_activation() {
    let captured_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let (mut instance, rails) = fixture(
        "runtime.component.scope.prepare",
        &[],
        [definition("component.a", {
            let captured_scope = Arc::clone(&captured_scope);
            move |scope| {
                let component_scope = scope.component_scope();
                assert_eq!(component_scope.participation(), scope.participation());
                assert_eq!(component_scope.component(), scope.component());
                assert!(matches!(
                    component_scope.begin_invocation(),
                    Err(ComponentError::ComponentUnavailable(_))
                ));
                assert!(matches!(
                    component_scope.update_health(Health::Degraded),
                    Err(ComponentError::ComponentParticipationNotActive(_))
                ));
                *captured_scope.lock().expect("captured scope") = Some(component_scope);
                Ok(Health::Healthy)
            }
        })],
    );

    let status = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize");
    let scope = captured_scope
        .lock()
        .expect("captured scope")
        .clone()
        .expect("component scope");
    let invocation = scope.begin_invocation().expect("component root");

    assert_eq!(scope.participation(), status.participation());
    assert_eq!(invocation.participation(), status.participation());
    assert_eq!(
        invocation.context().generation(),
        status.participation().generation()
    );
    assert_eq!(
        invocation.context().generation(),
        rails
            .runtime
            .current_status()
            .generation()
            .expect("runtime generation")
    );
    assert_eq!(
        invocation.context().root_origin(),
        &InvocationOrigin::Component(component(&rails, "component.a"))
    );

    instance.stop();
}

#[test]
fn continue_gateway_context_and_nested_operation_preserve_root_and_identity() {
    let scope_a: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let scope_b: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let (mut instance, rails) = fixture(
        "runtime.component.scope.gateway",
        &[],
        [
            definition("component.a", {
                let scope_a = Arc::clone(&scope_a);
                move |scope| {
                    let component_scope = scope.component_scope();
                    *scope_a.lock().expect("scope a") = Some(component_scope.clone());
                    scope.operation_with_context(operation_a(), move |context, input| {
                        let component_scope = component_scope.clone();
                        async move {
                            let invocation = component_scope.continue_invocation(&context)?;
                            let nested = invocation.invoke(&operation_b(), input).await?;
                            Ok(NestedContextOutput {
                                outer: context,
                                inner: nested.context,
                            })
                        }
                    })?;
                    Ok(Health::Healthy)
                }
            }),
            definition("component.b", {
                let scope_b = Arc::clone(&scope_b);
                move |scope| {
                    *scope_b.lock().expect("scope b") = Some(scope.component_scope());
                    scope.operation_with_context(operation_b(), |context, input| async move {
                        Ok(ContextOutput {
                            context,
                            reply: input.0,
                        })
                    })?;
                    Ok(Health::Healthy)
                }
            }),
        ],
    );

    rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize a");
    rails
        .materializer
        .materialize(&ComponentId::new("component.b").expect("component id"))
        .expect("materialize b");

    let response = block_on(
        rails
            .gateway
            .invoke(GatewayRequest::new(operation_a(), ContextInput("nested"))),
    )
    .expect("gateway invoke");
    let output = response.output();

    assert_eq!(output.outer.invocation_id(), output.inner.invocation_id());
    assert_eq!(output.outer.generation(), output.inner.generation());
    assert_eq!(output.outer.root_origin(), &InvocationOrigin::External);
    assert_eq!(output.inner.root_origin(), &InvocationOrigin::External);
    assert!(scope_a.lock().expect("scope a").is_some());
    assert!(scope_b.lock().expect("scope b").is_some());

    instance.stop();
}

#[test]
fn component_root_invocation_preserves_origin_for_operations_and_contract_calls() {
    let captured_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let (mut instance, rails) = fixture(
        "runtime.component.scope.component-root",
        &[],
        [
            definition("component.a", {
                let captured_scope = Arc::clone(&captured_scope);
                move |scope| {
                    *captured_scope.lock().expect("captured scope") = Some(scope.component_scope());
                    Ok(Health::Healthy)
                }
            }),
            definition("component.b", |scope| {
                scope.operation_with_context(operation_b(), |context, input| async move {
                    Ok(ContextOutput {
                        context,
                        reply: input.0,
                    })
                })?;
                Ok(Health::Healthy)
            }),
            definition("component.provider", |_| Ok(Health::Healthy)),
        ],
    );

    rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize a");
    rails
        .materializer
        .materialize(&ComponentId::new("component.b").expect("component id"))
        .expect("materialize b");
    rails
        .materializer
        .materialize(&ComponentId::new("component.provider").expect("component id"))
        .expect("materialize provider");

    let notes = bind_notes(
        &rails,
        component(&rails, "component.a"),
        component(&rails, "component.provider"),
    );
    let scope = captured_scope
        .lock()
        .expect("captured scope")
        .clone()
        .expect("component scope");
    let invocation = scope.begin_invocation().expect("begin component root");
    let nested = block_on(invocation.invoke(&operation_b(), ContextInput("component-root")))
        .expect("invoke nested operation");
    let notes_reply = invocation
        .call(&notes, |context, notes| NotesReply {
            context: context.clone(),
            reply: notes.reply(context, "notes"),
        })
        .expect("call notes");

    let expected_origin = InvocationOrigin::Component(component(&rails, "component.a"));
    assert_eq!(invocation.context().root_origin(), &expected_origin);
    assert_eq!(nested.context.root_origin(), &expected_origin);
    assert_eq!(notes_reply.context.root_origin(), &expected_origin);
    assert_eq!(
        invocation.context().invocation_id(),
        nested.context.invocation_id()
    );
    assert_eq!(
        invocation.context().invocation_id(),
        notes_reply.context.invocation_id()
    );

    instance.stop();
}

#[test]
fn scoped_contract_calls_preserve_consumer_and_availability_laws() {
    let scope_a: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let scope_b: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let scope_provider: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let (mut instance, rails) = fixture(
        "runtime.component.scope.contract",
        &[],
        [
            definition("component.a", {
                let scope_a = Arc::clone(&scope_a);
                move |scope| {
                    *scope_a.lock().expect("scope a") = Some(scope.component_scope());
                    Ok(Health::Healthy)
                }
            }),
            definition("component.b", {
                let scope_b = Arc::clone(&scope_b);
                move |scope| {
                    *scope_b.lock().expect("scope b") = Some(scope.component_scope());
                    Ok(Health::Healthy)
                }
            }),
            definition("component.provider", {
                let scope_provider = Arc::clone(&scope_provider);
                move |scope| {
                    *scope_provider.lock().expect("scope provider") = Some(scope.component_scope());
                    Ok(Health::Healthy)
                }
            }),
        ],
    );

    for component_id in ["component.a", "component.b", "component.provider"] {
        rails
            .materializer
            .materialize(&ComponentId::new(component_id).expect("component id"))
            .expect("materialize component");
    }

    let contract = bind_notes(
        &rails,
        component(&rails, "component.a"),
        component(&rails, "component.provider"),
    );
    let scope_a = scope_a.lock().expect("scope a").clone().expect("scope a");
    let scope_b = scope_b.lock().expect("scope b").clone().expect("scope b");
    let scope_provider = scope_provider
        .lock()
        .expect("scope provider")
        .clone()
        .expect("scope provider");

    let wrong_consumer = scope_b.begin_invocation().expect("scope b invocation");
    assert!(matches!(
        wrong_consumer.call(&contract, |_, notes| notes
            .reply(&wrong_consumer.context().clone(), "wrong")),
        Err(ComponentError::ComponentContractConsumerMismatch { .. })
    ));

    let invocation = scope_a.begin_invocation().expect("scope a invocation");
    scope_provider
        .update_health(Health::Unavailable)
        .expect("provider unavailable");
    assert!(matches!(
        invocation.call(&contract, |context, notes| notes.reply(context, "provider-down")),
        Err(ComponentError::ComponentUnavailable(component_id))
            if component_id == ComponentId::new("component.provider").expect("component id")
    ));

    scope_provider
        .update_health(Health::Healthy)
        .expect("provider healthy");
    scope_a
        .update_health(Health::Unavailable)
        .expect("caller unavailable");
    assert!(matches!(
        invocation.call(&contract, |context, notes| notes.reply(context, "caller-down")),
        Err(ComponentError::ComponentUnavailable(component_id))
            if component_id == ComponentId::new("component.a").expect("component id")
    ));

    instance.stop();
}

#[test]
fn active_scope_health_reporting_updates_runtime_truth_and_recovers() {
    let captured_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let (mut instance, rails) = fixture(
        "runtime.component.scope.health",
        &["component.a"],
        [definition("component.a", {
            let captured_scope = Arc::clone(&captured_scope);
            move |scope| {
                *captured_scope.lock().expect("captured scope") = Some(scope.component_scope());
                Ok(Health::Healthy)
            }
        })],
    );

    let status = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize a");
    let scope = captured_scope
        .lock()
        .expect("captured scope")
        .clone()
        .expect("component scope");

    let degraded = scope
        .update_health(Health::Degraded)
        .expect("degraded health");
    assert_eq!(degraded.participation(), status.participation());
    assert_eq!(degraded.health(), Health::Degraded);
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Degraded
    );
    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Degraded
    );

    let unavailable = scope
        .update_health(Health::Unavailable)
        .expect("unavailable health");
    assert_eq!(unavailable.health(), Health::Unavailable);
    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );

    let recovered = scope
        .update_health(Health::Healthy)
        .expect("recover health");
    assert_eq!(recovered.health(), Health::Healthy);
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );
    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Healthy
    );

    instance.stop();
}

#[test]
fn stale_and_fresh_runtime_scopes_remain_fenced() {
    let first_scope_capture: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let second_scope_capture: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let (mut instance_one, rails_one) = fixture(
        "runtime.component.scope.shared",
        &[],
        [definition("component.a", {
            let first_scope_capture = Arc::clone(&first_scope_capture);
            move |scope| {
                *first_scope_capture.lock().expect("first scope") = Some(scope.component_scope());
                Ok(Health::Healthy)
            }
        })],
    );

    rails_one
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize first");
    let first_scope = first_scope_capture
        .lock()
        .expect("first scope")
        .clone()
        .expect("first scope");
    let first_invocation = first_scope.begin_invocation().expect("first invocation");
    let stopped = rails_one
        .materializer
        .dematerialize(&ComponentId::new("component.a").expect("component id"))
        .expect("dematerialize first");
    assert!(matches!(
        first_scope.update_health(Health::Degraded),
        Err(ComponentError::UnknownComponent(_))
            | Err(ComponentError::StaleComponentParticipation(_))
    ));
    assert!(matches!(
        first_scope.begin_invocation(),
        Err(ComponentError::InvocationOriginNotParticipating(_))
            | Err(ComponentError::ComponentUnavailable(_))
    ));

    rails_one
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize second");
    let current = rails_one
        .registry
        .component(&ComponentId::new("component.a").expect("component id"))
        .expect("current component");
    assert_ne!(stopped.participation(), current.participation());
    assert!(matches!(
        first_scope.continue_invocation(first_invocation.context()),
        Err(ComponentError::InvocationOriginNotParticipating(_))
            | Err(ComponentError::ComponentUnavailable(_))
    ));

    let (mut instance_two, rails_two) = fixture(
        "runtime.component.scope.shared",
        &[],
        [definition("component.a", {
            let second_scope_capture = Arc::clone(&second_scope_capture);
            move |scope| {
                *second_scope_capture.lock().expect("second scope") = Some(scope.component_scope());
                Ok(Health::Healthy)
            }
        })],
    );
    rails_two
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize fresh runtime");
    let second_scope = second_scope_capture
        .lock()
        .expect("second scope")
        .clone()
        .expect("second scope");
    let second_invocation = second_scope.begin_invocation().expect("second invocation");

    assert!(matches!(
        first_scope.continue_invocation(second_invocation.context()),
        Err(ComponentError::InvocationContextGenerationMismatch { .. })
    ));

    instance_one.stop();
    instance_two.stop();
}
