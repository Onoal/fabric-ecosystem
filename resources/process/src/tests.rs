use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    module_factory,
};
use fabric_projection::{ProjectionLease, ProjectionLeases};
use fabric_resource_registry::{ResourceRegistry, ResourceRegistryModule};

use crate::{
    NativeProcess, PreparedProcess, PreparedProcessEnvironment, ProcessAdapter, ProcessArtifact,
    ProcessContract, ProcessEntrypoint, ProcessError, ProcessExecution, ProcessExecutionRequest,
    ProcessId, ProcessInstanceId, ProcessSpec, StartProcessRequest, StopProcessRequest,
    process_contract_id, process_resource_id,
};

#[derive(Default)]
struct Capture {
    process: Mutex<Option<ProcessContract>>,
    registry: Mutex<Option<Arc<ResourceRegistry>>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    process_requirement: ContractRequirement<ProcessContract>,
    registry_requirement: ContractRequirement<ResourceRegistry>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.process.capture").expect("module id"),
            process_requirement: ContractRequirement::provisional(process_contract_id()),
            registry_requirement: ContractRequirement::provisional(
                fabric_resource_registry::resource_registry_contract_id(),
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
        vec![self.process_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.registry_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let process = bindings
            .resolve(&self.process_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.process.lock().expect("process capture") = Some((*process).clone());
        let registry = bindings
            .resolve_optional(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.registry.lock().expect("registry capture") = registry;
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

#[derive(Default)]
struct AdapterState {
    prepared: Vec<ProcessSpec>,
    started: Vec<(ProcessInstanceId, ProcessSpec, BTreeMap<String, String>)>,
    stopped: Vec<ProcessInstanceId>,
    cleaned: usize,
}

#[derive(Clone)]
struct FakeProcessAdapter {
    state: Arc<Mutex<AdapterState>>,
}

impl ProcessAdapter for FakeProcessAdapter {
    fn prepare(&mut self, process: &ProcessSpec) -> Result<(), ProcessError> {
        self.state
            .lock()
            .expect("adapter state")
            .prepared
            .push(process.clone());
        Ok(())
    }

    fn start(
        &mut self,
        request: ProcessExecutionRequest,
    ) -> Result<Box<dyn ProcessExecution>, ProcessError> {
        let (environment, leases) = request.start.environment.into_parts();
        self.state.lock().expect("adapter state").started.push((
            request.instance_id.clone(),
            request.start.process.clone(),
            environment,
        ));
        Ok(Box::new(FakeExecution {
            process_instance_id: request.instance_id,
            state: Arc::clone(&self.state),
            projection_leases: leases,
        }))
    }

    fn clear(&mut self) {}
}

struct FakeExecution {
    process_instance_id: ProcessInstanceId,
    state: Arc<Mutex<AdapterState>>,
    projection_leases: ProjectionLeases,
}

impl ProcessExecution for FakeExecution {
    fn stop(&mut self) -> Result<(), ProcessError> {
        self.state
            .lock()
            .expect("adapter state")
            .stopped
            .push(self.process_instance_id.clone());
        Ok(())
    }

    fn cleanup(&mut self) {
        self.state.lock().expect("adapter state").cleaned += 1;
        self.projection_leases.release_all();
    }
}

struct TestLease(Arc<Mutex<bool>>);

impl ProjectionLease for TestLease {
    fn release(self: Box<Self>) {
        *self.0.lock().expect("released") = true;
    }
}

fn process_module(state: Arc<Mutex<AdapterState>>) -> impl fabric_core::Module {
    module_factory(move || {
        NativeProcess::with_adapter(Box::new(FakeProcessAdapter {
            state: Arc::clone(&state),
        }))
    })
}

#[test]
fn process_resource_is_adapter_and_registry_open() {
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.process.resource".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("process".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
            .register_module(process_module(Arc::clone(&adapter_state)))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(InstanceId::new("fabric.process.resource".to_owned()).expect("instance id"))
        .expect("materialize");
    instance.start().expect("start");

    let registry = capture
        .registry
        .lock()
        .expect("registry capture")
        .clone()
        .expect("captured registry");
    let resources = registry.resources();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].resource_id(), &process_resource_id());
    assert_eq!(
        resources[0].module_id().as_str(),
        "fabric.resource.process.native"
    );
    assert_eq!(
        resources[0].provided_contracts()[0].as_str(),
        "fabric.resource.process"
    );

    let process = capture
        .process
        .lock()
        .expect("process capture")
        .clone()
        .expect("captured process");
    let spec = test_process_spec("notes.run");
    let prepared = process
        .prepare_process(spec.clone())
        .expect("prepare process");
    let instance = process
        .start_process(StartProcessRequest {
            prepared_process: prepared,
            process: spec.clone(),
            environment: PreparedProcessEnvironment::default(),
        })
        .expect("start process");
    assert_eq!(instance.process_id, spec.process_id);
    process
        .stop_process(StopProcessRequest {
            process_instance_id: instance.process_instance_id,
        })
        .expect("stop process");
}

#[test]
fn external_consumer_uses_only_process_contract() {
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.process.consumer".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("process".to_owned()).expect("block id"))
            .register_module(process_module(Arc::clone(&adapter_state)))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(InstanceId::new("fabric.process.consumer".to_owned()).expect("instance id"))
        .expect("materialize");
    instance.start().expect("start");

    let process = capture
        .process
        .lock()
        .expect("process capture")
        .clone()
        .expect("captured process");
    let spec = test_process_spec("external.consumer");
    let prepared = process
        .prepare_process(spec.clone())
        .expect("prepare process");
    let mut environment = PreparedProcessEnvironment::default();
    environment
        .insert_public("FABRIC_MODE", "external")
        .expect("environment");
    let instance = process
        .start_process(StartProcessRequest {
            prepared_process: prepared,
            process: spec.clone(),
            environment,
        })
        .expect("start process");
    process
        .stop_process(StopProcessRequest {
            process_instance_id: instance.process_instance_id.clone(),
        })
        .expect("stop process");

    let state = adapter_state.lock().expect("adapter state");
    assert_eq!(state.prepared.len(), 1);
    assert_eq!(state.started.len(), 1);
    assert_eq!(state.started[0].1.process_id.as_str(), "external.consumer");
    assert_eq!(
        state.started[0].2.get("FABRIC_MODE"),
        Some(&"external".to_owned())
    );
    assert_eq!(state.stopped, vec![instance.process_instance_id]);
}

#[test]
fn process_environment_releases_projection_leases_on_stop_and_failed_start() {
    let released = Arc::new(Mutex::new(false));
    let mut environment = PreparedProcessEnvironment::default();
    let mut leases = ProjectionLeases::default();
    leases.push(TestLease(Arc::clone(&released)));
    environment.append_leases(leases);

    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let mut adapter = FakeProcessAdapter {
        state: Arc::clone(&adapter_state),
    };
    let spec = test_process_spec("lease.process");
    adapter.prepare(&spec).expect("prepare");
    let request = ProcessExecutionRequest {
        instance_id: ProcessInstanceId::new("proc-1").expect("process instance id"),
        start: StartProcessRequest {
            prepared_process: PreparedProcess::new(spec.clone()),
            process: spec,
            environment,
        },
    };
    let mut execution = adapter.start(request).expect("start");
    assert!(!*released.lock().expect("released"));
    execution.stop().expect("stop");
    execution.cleanup();
    assert!(*released.lock().expect("released"));
}

#[test]
fn process_resource_public_model_is_process_domain_only() {
    assert!(ProcessId::new("notes.process").is_ok());
    assert!(ProcessEntrypoint::new("bin/run.sh").is_ok());
    assert!(ProcessInstanceId::new("proc_01").is_ok());
}

fn test_process_spec(process_id: &str) -> ProcessSpec {
    ProcessSpec {
        process_id: ProcessId::new(process_id).expect("process id"),
        artifact: ProcessArtifact {
            reference: format!("fabric://artifact/{process_id}"),
            sha256: [7; 32],
        },
        entrypoint: ProcessEntrypoint::new("run.sh").expect("entrypoint"),
    }
}
