use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};

use fabric_component::{
    Component, ComponentControlRail, ComponentControlSnapshot, ComponentError, ComponentId,
    ComponentMaterializer, ComponentReadinessPolicy, ComponentReconstructionRail,
    ComponentRegistry, ComponentRuntime, ComponentRuntimeDefinition, ComponentRuntimeModule,
    ComponentRuntimeScope, InvocationRail, OperationDescriptor, OperationId, OperationKey,
    OperationRail, OperationRegistrar, OperationTypeId,
};
use fabric_component_gateway::{Gateway, GatewayModule, GatewayRequest, GatewayResponse};
use fabric_component_namespace::NamespaceModule;
use fabric_component_publication::PublicationModule;

const OPERATION_A_ID: &str = "fabric.component.catalog.a";
const OPERATION_B_ID: &str = "fabric.component.catalog.b";
const OPERATION_C_ID: &str = "fabric.component.catalog.c";
#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoOutput(&'static str);

struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    registrar: Arc<OperationRegistrar>,
    operations: Arc<OperationRail>,
    gateway: Arc<Gateway>,
    invocation: Arc<InvocationRail>,
    control: Arc<ComponentControlRail>,
    reconstruction: Arc<ComponentReconstructionRail>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    registrar: ContractRequirement<OperationRegistrar>,
    operations: ContractRequirement<OperationRail>,
    gateway: ContractRequirement<Gateway>,
    invocation: ContractRequirement<InvocationRail>,
    control: ContractRequirement<ComponentControlRail>,
    reconstruction: ContractRequirement<ComponentReconstructionRail>,
    materializer: ContractRequirement<ComponentMaterializer>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.operation.catalog.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
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
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            control: ContractRequirement::provisional(
                fabric_component::component_control_contract_id(),
            ),
            reconstruction: ContractRequirement::provisional(
                fabric_component::component_reconstruction_contract_id(),
            ),
            materializer: ContractRequirement::provisional(
                fabric_component::component_materializer_contract_id(),
            ),
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
            self.registrar.id().clone(),
            self.operations.id().clone(),
            self.gateway.id().clone(),
            self.invocation.id().clone(),
            self.control.id().clone(),
            self.reconstruction.id().clone(),
            self.materializer.id().clone(),
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
            registrar: bindings.resolve(&self.registrar).map_err(module_error)?,
            operations: bindings.resolve(&self.operations).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
            invocation: bindings.resolve(&self.invocation).map_err(module_error)?,
            control: bindings.resolve(&self.control).map_err(module_error)?,
            reconstruction: bindings
                .resolve(&self.reconstruction)
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

fn component(runtime_contract: &ComponentRuntime, component_id: &str) -> Component {
    Component::bind(
        ComponentId::new(component_id).expect("component id"),
        runtime_contract,
    )
}

fn operation_key(
    operation_id: &str,
    input_type: &str,
    output_type: &str,
) -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(operation_id).expect("operation id"),
        OperationTypeId::new(input_type).expect("input type id"),
        OperationTypeId::new(output_type).expect("output type id"),
    )
}

fn definition_with_operation(
    component_id: &str,
    operation: OperationKey<EchoInput, EchoOutput>,
) -> ComponentRuntimeDefinition {
    ComponentRuntimeDefinition::new(
        ComponentId::new(component_id).expect("component id"),
        move |scope: &ComponentRuntimeScope| {
            let operation = operation.clone();
            scope.operation_with_context(operation, |_context, input: EchoInput| async move {
                Ok(EchoOutput(input.0))
            })?;
            Ok(Health::Healthy)
        },
    )
}

fn fixture(
    instance_id: &str,
    runtime_definitions: impl IntoIterator<Item = ComponentRuntimeDefinition>,
    control_snapshot: Option<ComponentControlSnapshot>,
) -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let module = match control_snapshot {
        Some(control_snapshot) => ComponentRuntimeModule::with_configuration_and_control_snapshot(
            ComponentReadinessPolicy::empty(),
            runtime_definitions,
            control_snapshot,
        )
        .expect("component runtime with snapshot"),
        None => ComponentRuntimeModule::with_configuration(
            ComponentReadinessPolicy::empty(),
            runtime_definitions,
        )
        .expect("component runtime"),
    };
    let composition = CompositionBuilder::new(
        CompositionId::new("runtime.operation.catalog").expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("runtime.operation.catalog.block").expect("block id"))
            .register_module(module)
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

fn descriptor_ids(descriptor: &OperationDescriptor) -> (&str, &str, &str, &str) {
    (
        descriptor.definition().id().as_str(),
        descriptor.definition().input_type().as_str(),
        descriptor.definition().output_type().as_str(),
        descriptor.owner().component_id().as_str(),
    )
}

#[test]
fn catalog_visibility_and_health_follow_activation_without_changing_definition_identity() {
    let (mut composition, rails) = fixture("runtime.catalog.visibility", Vec::new(), None);
    let owner = component(rails.runtime.as_ref(), "component.catalog");
    let participation = rails
        .registry
        .register(owner.clone(), Health::Healthy)
        .expect("register owner")
        .participation()
        .clone();
    let operation = operation_key(
        OPERATION_A_ID,
        "fabric.test.catalog.echo.input",
        "fabric.test.catalog.echo.output",
    );

    assert_eq!(operation.definition().id().as_str(), OPERATION_A_ID);
    assert_eq!(
        operation.definition().input_type().as_str(),
        "fabric.test.catalog.echo.input"
    );
    assert_eq!(
        operation.definition().output_type().as_str(),
        "fabric.test.catalog.echo.output"
    );

    rails
        .registrar
        .register(
            participation.clone(),
            operation.clone(),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        )
        .expect("register preparing operation");

    assert!(rails.operations.operations().is_empty());
    assert_eq!(
        rails.operations.operation(operation.id()),
        Err(ComponentError::UnknownOperation(operation.id().clone()))
    );

    rails
        .registry
        .activate(&participation)
        .expect("activate owner");
    let descriptor = rails
        .operations
        .operation(operation.id())
        .expect("active descriptor");
    assert_eq!(
        descriptor_ids(&descriptor),
        (
            OPERATION_A_ID,
            "fabric.test.catalog.echo.input",
            "fabric.test.catalog.echo.output",
            "component.catalog",
        )
    );
    assert_eq!(rails.operations.operations(), vec![descriptor.clone()]);

    let result = block_on(rails.operations.invoke_with_context(
        rails.invocation.begin_external().expect("gateway root"),
        &operation,
        EchoInput("ok"),
    ))
    .expect("invoke active operation");
    assert_eq!(result, EchoOutput("ok"));

    rails
        .registry
        .update_health(&participation, Health::Unavailable)
        .expect("set unavailable");
    assert_eq!(
        rails
            .operations
            .operation(operation.id())
            .expect("still catalogued"),
        descriptor
    );
    assert_eq!(
        block_on(rails.operations.invoke_with_context(
            rails.invocation.begin_external().expect("gateway root"),
            &operation,
            EchoInput("down"),
        )),
        Err(ComponentError::ComponentUnavailable(
            owner.component_id().clone()
        ))
    );

    rails
        .registry
        .update_health(&participation, Health::Healthy)
        .expect("recover health");
    assert_eq!(
        block_on(
            rails
                .gateway
                .invoke(GatewayRequest::new(operation.clone(), EchoInput("gateway"),))
        ),
        Ok(GatewayResponse::new(EchoOutput("gateway")))
    );

    rails
        .registry
        .unregister(&participation)
        .expect("unregister owner");
    assert!(rails.operations.operations().is_empty());
    assert_eq!(
        rails.operations.operation(operation.id()),
        Err(ComponentError::UnknownOperation(operation.id().clone()))
    );

    let second = rails
        .registry
        .register(owner.clone(), Health::Healthy)
        .expect("register owner again")
        .participation()
        .clone();
    assert_ne!(participation, second);
    rails
        .registrar
        .register(
            second.clone(),
            operation.clone(),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        )
        .expect("register same definition again");
    rails.registry.activate(&second).expect("activate second");
    assert_eq!(
        rails
            .operations
            .operation(operation.id())
            .expect("descriptor after rejoin")
            .definition(),
        descriptor.definition()
    );

    composition.stop();
}

#[test]
fn catalog_enumeration_is_deterministic_and_transport_neutral() {
    let (mut composition, rails) = fixture("runtime.catalog.order", Vec::new(), None);
    let owner_b = component(rails.runtime.as_ref(), "component.catalog.b");
    let owner_a = component(rails.runtime.as_ref(), "component.catalog.a");
    let participation_b = rails
        .registry
        .register(owner_b, Health::Healthy)
        .expect("register b")
        .participation()
        .clone();
    let participation_a = rails
        .registry
        .register(owner_a, Health::Healthy)
        .expect("register a")
        .participation()
        .clone();
    let operation_b = operation_key(
        OPERATION_B_ID,
        "fabric.test.catalog.order.b.input",
        "fabric.test.catalog.order.b.output",
    );
    let operation_a = operation_key(
        OPERATION_A_ID,
        "fabric.test.catalog.order.a.input",
        "fabric.test.catalog.order.a.output",
    );

    rails
        .registrar
        .register(
            participation_b.clone(),
            operation_b.clone(),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        )
        .expect("register b");
    rails
        .registrar
        .register(
            participation_a.clone(),
            operation_a.clone(),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        )
        .expect("register a");
    rails
        .registry
        .activate(&participation_b)
        .expect("activate b");
    rails
        .registry
        .activate(&participation_a)
        .expect("activate a");

    let ids = rails
        .operations
        .operations()
        .into_iter()
        .map(|descriptor| descriptor.definition().id().as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![OPERATION_A_ID.to_owned(), OPERATION_B_ID.to_owned()]
    );
    assert_eq!(
        block_on(
            rails
                .gateway
                .invoke(GatewayRequest::new(operation_a, EchoInput("gateway"),))
        ),
        Ok(GatewayResponse::new(EchoOutput("gateway")))
    );

    composition.stop();
    assert!(rails.operations.operations().is_empty());
}

#[test]
fn fresh_shell_reconstruction_replays_equivalent_operation_descriptors() {
    let operation = operation_key(
        OPERATION_C_ID,
        "fabric.test.catalog.snapshot.input",
        "fabric.test.catalog.snapshot.output",
    );
    let runtime_definition = definition_with_operation("component.catalog.snapshot", operation);
    let (mut composition_one, rails_one) = fixture(
        "runtime.catalog.snapshot",
        vec![runtime_definition.clone()],
        None,
    );
    let component_a = component(rails_one.runtime.as_ref(), "component.catalog.snapshot");
    rails_one
        .control
        .enable(component_a.clone())
        .expect("enable component");
    let first_report = rails_one
        .reconstruction
        .reconstruct()
        .expect("reconstruct one");
    assert_eq!(first_report.outcomes().len(), 1);
    let snapshot = rails_one.control.snapshot();
    let descriptor_one = rails_one
        .operations
        .operation(&OperationId::new(OPERATION_C_ID).expect("operation id"))
        .expect("descriptor one");

    composition_one.stop();

    let (mut composition_two, rails_two) = fixture(
        "runtime.catalog.snapshot",
        vec![runtime_definition],
        Some(snapshot.clone()),
    );
    assert!(rails_two.operations.operations().is_empty());
    assert_eq!(rails_two.control.snapshot(), snapshot);

    let second_report = rails_two
        .reconstruction
        .reconstruct()
        .expect("reconstruct two");
    assert_eq!(second_report.outcomes().len(), 1);
    let descriptor_two = rails_two
        .operations
        .operation(&OperationId::new(OPERATION_C_ID).expect("operation id"))
        .expect("descriptor two");

    assert_eq!(descriptor_one.definition(), descriptor_two.definition());
    assert_eq!(
        descriptor_one.owner().component_id(),
        descriptor_two.owner().component_id()
    );
    assert_eq!(
        descriptor_one.owner().instance_id(),
        descriptor_two.owner().instance_id()
    );

    composition_two.stop();
}
