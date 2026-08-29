use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    module_factory,
};
use fabric_resource_process::{
    NativeProcess, PreparedProcessEnvironment, ProcessAdapter, ProcessArtifact, ProcessContract,
    ProcessEntrypoint, ProcessError, ProcessExecution, ProcessExecutionRequest, ProcessId,
    ProcessInstanceId, ProcessSpec, StartProcessRequest, StopProcessRequest, process_contract_id,
};

#[derive(Default)]
struct AdapterState {
    prepared: Vec<ProcessSpec>,
    started: Vec<(ProcessInstanceId, ProcessSpec, BTreeMap<String, String>)>,
    stopped: Vec<ProcessInstanceId>,
    cleaned: usize,
}

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
        let (environment, _leases) = request.start.environment.into_parts();
        self.state.lock().expect("adapter state").started.push((
            request.instance_id.clone(),
            request.start.process.clone(),
            environment,
        ));
        Ok(Box::new(FakeExecution {
            process_instance_id: request.instance_id,
            state: Arc::clone(&self.state),
        }))
    }

    fn clear(&mut self) {}
}

struct FakeExecution {
    process_instance_id: ProcessInstanceId,
    state: Arc<Mutex<AdapterState>>,
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
    }
}

#[derive(Default)]
struct Capture {
    process: Mutex<Option<ProcessContract>>,
}

#[derive(Clone)]
struct ConsumerModule {
    module_id: ModuleId,
    process_requirement: ContractRequirement<ProcessContract>,
    capture: Arc<Capture>,
}

impl ConsumerModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.process.consumer").expect("module id"),
            process_requirement: ContractRequirement::provisional(process_contract_id()),
            capture,
        }
    }
}

impl ModuleRuntime for ConsumerModule {
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

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let process = bindings
            .resolve(&self.process_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.process.lock().expect("process capture") = Some((*process).clone());
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

#[test]
fn external_consumer_uses_process_contract_without_worker_or_systemd_imports() {
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.test.process.vertical".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("process".to_owned()).expect("block id"))
            .register_module(module_factory({
                let adapter_state = Arc::clone(&adapter_state);
                move || {
                    NativeProcess::with_adapter(Box::new(FakeProcessAdapter {
                        state: Arc::clone(&adapter_state),
                    }))
                }
            }))
            .register_module(ConsumerModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(
            InstanceId::new("fabric.test.process.vertical".to_owned()).expect("instance id"),
        )
        .expect("materialize");
    instance.start().expect("start");

    let process = capture
        .process
        .lock()
        .expect("process capture")
        .clone()
        .expect("captured process");
    let spec = test_process_spec("notes.process");
    let prepared = process
        .prepare_process(spec.clone())
        .expect("prepare process");
    let mut environment = PreparedProcessEnvironment::default();
    environment
        .insert_public("FABRIC_PROCESS_MODE", "vertical")
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
    assert_eq!(state.started[0].1.process_id, spec.process_id);
    assert_eq!(
        state.started[0].2.get("FABRIC_PROCESS_MODE"),
        Some(&"vertical".to_owned())
    );
    assert_eq!(state.stopped, vec![instance.process_instance_id]);
}

fn test_process_spec(process_id: &str) -> ProcessSpec {
    ProcessSpec {
        process_id: ProcessId::new(process_id).expect("process id"),
        artifact: ProcessArtifact {
            reference: format!("fabric://artifact/{process_id}"),
            sha256: [9; 32],
        },
        entrypoint: ProcessEntrypoint::new("run.sh").expect("entrypoint"),
    }
}
