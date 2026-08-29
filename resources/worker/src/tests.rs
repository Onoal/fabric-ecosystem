use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, InstanceId,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime, module_factory,
};
use fabric_resource_authority::ActorRef;
use fabric_resource_database::DatabaseRef;
use fabric_resource_secrets::{AuthorizedSecretRef, SecretId, SecretRef};
use fabric_resource_service::ServiceId;

use crate::{
    BindingName, BindingProjection, BindingTarget, NativeWorker, PreparedWorker,
    StartWorkerRequest, WorkerAdapter, WorkerApiVersion, WorkerContract, WorkerError,
    WorkerExecution, WorkerExecutionRequest, WorkerFeature, WorkerFeatureSupport,
    WorkerRequirement, WorkerSpec, WorkloadArtifact, WorkloadBinding, WorkloadBindingEnv,
    WorkloadBindingProjection, WorkloadEntrypoint, WorkloadId,
};

#[derive(Default)]
struct Capture {
    worker: Mutex<Option<WorkerContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    requirement: ContractRequirement<WorkerContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.worker.capture").expect("module id"),
            requirement: ContractRequirement::provisional(crate::worker_contract_id()),
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
        vec![self.requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let worker = bindings
            .resolve(&self.requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.worker.lock().expect("capture lock") = Some((*worker).clone());
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn health(&self) -> fabric_core::Health {
        fabric_core::Health::Healthy
    }
}

struct TestExecution;

impl WorkerExecution for TestExecution {
    fn stop(&mut self) -> Result<(), WorkerError> {
        Ok(())
    }

    fn cleanup(&mut self) {}
}

struct TestWorkerAdapter;

fn test_worker_capabilities() -> crate::WorkerCapabilities {
    crate::WorkerCapabilities {
        api_versions: vec![WorkerApiVersion::parse("1.0.0").expect("api version")],
        features: vec![
            WorkerFeatureSupport {
                feature: WorkerFeature::new("js.module").expect("feature"),
                supported: true,
            },
            WorkerFeatureSupport {
                feature: WorkerFeature::new("http.fetch").expect("feature"),
                supported: true,
            },
        ],
    }
}

impl WorkerAdapter for TestWorkerAdapter {
    fn capabilities(&self) -> crate::WorkerCapabilities {
        test_worker_capabilities()
    }

    fn prepare(&mut self, _worker: &WorkerSpec) -> Result<(), WorkerError> {
        Ok(())
    }

    fn start(
        &mut self,
        _request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError> {
        Ok(Box::new(TestExecution))
    }

    fn clear(&mut self) {}
}

fn test_native_worker() -> NativeWorker {
    NativeWorker::with_adapter(Box::new(TestWorkerAdapter))
}

#[derive(Default)]
struct RealizationLifecycleState {
    prepares: Vec<WorkerSpec>,
    starts: usize,
}

struct NonValidatingRealization {
    state: Arc<Mutex<RealizationLifecycleState>>,
}

impl WorkerAdapter for NonValidatingRealization {
    fn capabilities(&self) -> crate::WorkerCapabilities {
        test_worker_capabilities()
    }

    fn prepare(&mut self, worker: &WorkerSpec) -> Result<(), WorkerError> {
        self.state
            .lock()
            .expect("lifecycle state")
            .prepares
            .push(worker.clone());
        Ok(())
    }

    fn start(
        &mut self,
        _request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError> {
        self.state.lock().expect("lifecycle state").starts += 1;
        Ok(Box::new(TestExecution))
    }

    fn clear(&mut self) {}
}

fn test_non_validating_worker(state: Arc<Mutex<RealizationLifecycleState>>) -> NativeWorker {
    NativeWorker::with_adapter(Box::new(NonValidatingRealization { state }))
}

struct FailingPrepareRealization;

impl WorkerAdapter for FailingPrepareRealization {
    fn capabilities(&self) -> crate::WorkerCapabilities {
        test_worker_capabilities()
    }

    fn prepare(&mut self, _worker: &WorkerSpec) -> Result<(), WorkerError> {
        Err(WorkerError::PrepareFailed {
            message: "synthetic prepare failure".to_owned(),
        })
    }

    fn start(
        &mut self,
        _request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError> {
        Ok(Box::new(TestExecution))
    }

    fn clear(&mut self) {}
}

fn test_failing_prepare_worker() -> NativeWorker {
    NativeWorker::with_adapter(Box::new(FailingPrepareRealization))
}

fn start_test_instance(
    composition_id: &str,
    composition: fabric_core::Composition,
) -> fabric_core::Instance {
    let mut instance = composition
        .materialize(InstanceId::new(composition_id).expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start composition");
    instance
}

#[test]
fn native_worker_exports_contract_and_capabilities() {
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.tests".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory(test_native_worker))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let _instance = start_test_instance("fabric.resource.worker.tests", composition);
    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    assert_eq!(
        worker
            .inspect_capabilities()
            .expect("capabilities")
            .api_versions
            .len(),
        1
    );
}

#[test]
fn native_worker_starts_runtime_neutral_workload() {
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.start".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory(test_native_worker))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.start", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let spec = test_worker_spec();
    let prepared = worker.prepare_worker(spec.clone()).expect("prepare worker");
    let instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: spec.clone(),
            bindings: vec![WorkloadBinding::new(
                spec.workload_id.clone(),
                BindingName::new("DB").expect("binding name"),
                BindingTarget::database(
                    DatabaseRef::parse(
                        "fabric-resource-database-ref-v1:fabric-resource-database-deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                    )
                    .expect("database ref"),
                ),
            )],
            binding_projections: Vec::new(),
        })
        .expect("start worker");
    assert_eq!(instance.workload_id, spec.workload_id);
}

#[test]
fn native_worker_constructs_canonical_prepared_truth_after_realization_prepare_succeeds() {
    let capture = Arc::new(Capture::default());
    let lifecycle = Arc::new(Mutex::new(RealizationLifecycleState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.prepare-truth".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory({
                let lifecycle = Arc::clone(&lifecycle);
                move || test_non_validating_worker(Arc::clone(&lifecycle))
            }))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.prepare-truth", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let spec = test_worker_spec();
    let prepared = worker.prepare_worker(spec.clone()).expect("prepare worker");
    let lifecycle = lifecycle.lock().expect("lifecycle state");
    assert_eq!(lifecycle.prepares, vec![spec.clone()]);
    assert_eq!(prepared, PreparedWorker::new(spec));
}

#[test]
fn native_worker_does_not_create_prepared_truth_when_realization_prepare_fails() {
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.prepare-failure".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory(test_failing_prepare_worker))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.prepare-failure", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let error = worker
        .prepare_worker(test_worker_spec())
        .expect_err("prepare failure should prevent canonical prepared worker");
    assert!(matches!(
        error,
        WorkerError::PrepareFailed { ref message } if message == "synthetic prepare failure"
    ));
}

#[test]
fn native_worker_rejects_start_when_prepared_workload_does_not_match() {
    let capture = Arc::new(Capture::default());
    let lifecycle = Arc::new(Mutex::new(RealizationLifecycleState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.workload-mismatch".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory({
                let lifecycle = Arc::clone(&lifecycle);
                move || test_non_validating_worker(Arc::clone(&lifecycle))
            }))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.workload-mismatch", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let prepared = worker
        .prepare_worker(test_worker_spec())
        .expect("prepare worker");
    let mismatched = test_worker_spec_with_workload("other.app:release");

    let error = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: mismatched,
            bindings: Vec::new(),
            binding_projections: Vec::new(),
        })
        .expect_err("mismatched start should fail");

    assert!(matches!(
        error,
        WorkerError::ProtocolViolation { ref message }
            if message.contains("prepared worker does not match start worker workload")
    ));
    assert_eq!(lifecycle.lock().expect("lifecycle state").starts, 0);
}

#[test]
fn native_worker_rejects_start_when_prepared_artifact_does_not_match() {
    let capture = Arc::new(Capture::default());
    let lifecycle = Arc::new(Mutex::new(RealizationLifecycleState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.artifact-mismatch".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory({
                let lifecycle = Arc::clone(&lifecycle);
                move || test_non_validating_worker(Arc::clone(&lifecycle))
            }))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.artifact-mismatch", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let prepared = worker
        .prepare_worker(test_worker_spec())
        .expect("prepare worker");
    let mut mismatched = test_worker_spec();
    mismatched.artifact = WorkloadArtifact {
        reference: "stel://artifact/other".to_owned(),
        sha256: [9; 32],
    };

    let error = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: mismatched,
            bindings: Vec::new(),
            binding_projections: Vec::new(),
        })
        .expect_err("mismatched start should fail");

    assert!(matches!(
        error,
        WorkerError::ProtocolViolation { ref message }
            if message.contains("prepared worker does not match start worker")
    ));
    assert_eq!(lifecycle.lock().expect("lifecycle state").starts, 0);
}

#[test]
fn native_worker_rejects_start_when_prepared_entrypoint_does_not_match() {
    let capture = Arc::new(Capture::default());
    let lifecycle = Arc::new(Mutex::new(RealizationLifecycleState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.entrypoint-mismatch".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory({
                let lifecycle = Arc::clone(&lifecycle);
                move || test_non_validating_worker(Arc::clone(&lifecycle))
            }))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.entrypoint-mismatch", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let prepared = worker
        .prepare_worker(test_worker_spec())
        .expect("prepare worker");
    let mut mismatched = test_worker_spec();
    mismatched.entrypoint = WorkloadEntrypoint::new("worker.ts").expect("entrypoint");

    let error = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: mismatched,
            bindings: Vec::new(),
            binding_projections: Vec::new(),
        })
        .expect_err("mismatched start should fail");

    assert!(matches!(
        error,
        WorkerError::ProtocolViolation { ref message }
            if message.contains("prepared worker does not match start worker")
    ));
    assert_eq!(lifecycle.lock().expect("lifecycle state").starts, 0);
}

#[test]
fn workload_binding_keeps_consumer_target_and_projection_distinct() {
    let workload = WorkloadId::new("apps.notes").expect("workload");
    let reference = DatabaseRef::parse(
        "fabric-resource-database-ref-v1:fabric-resource-database-deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
    )
    .expect("database ref");
    let binding = WorkloadBinding::new(
        workload.clone(),
        BindingName::new("DB").expect("binding name"),
        BindingTarget::database(reference.clone()),
    );
    let structured = WorkloadBindingProjection::structured(binding.binding_id());
    let projected = WorkloadBindingProjection::environment(
        binding.binding_id(),
        WorkloadBindingEnv::new("DATABASE_URL").expect("environment binding"),
    );

    assert_eq!(binding.consumer(), &workload);
    assert_eq!(
        binding.target(),
        &BindingTarget::database(reference.clone())
    );
    assert!(matches!(
        structured.projection(),
        BindingProjection::Structured
    ));
    assert!(matches!(
        projected.projection(),
        BindingProjection::Environment(variable) if variable.as_str() == "DATABASE_URL"
    ));
}

#[test]
fn workload_binding_rejects_foreign_consumer_context() {
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.invalid-binding".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory(test_native_worker))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.invalid-binding", composition);

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let spec = test_worker_spec();
    let prepared = worker.prepare_worker(spec.clone()).expect("prepare worker");
    let error = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: spec.clone(),
            bindings: vec![WorkloadBinding::new(
                WorkloadId::new("other.workload").expect("foreign workload"),
                BindingName::new("DB").expect("binding name"),
                BindingTarget::database(
                    DatabaseRef::parse(
                        "fabric-resource-database-ref-v1:fabric-resource-database-deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                    )
                    .expect("database ref"),
                ),
            )],
            binding_projections: Vec::new(),
        })
        .expect_err("foreign binding should fail");
    assert!(matches!(
        error,
        WorkerError::InvalidInput { ref message }
        if message.contains("belongs to workload other.workload")
            && message.contains(spec.workload_id.as_str())
    ));
}

#[test]
fn workload_binding_rejects_duplicate_bindings_and_environment_projections() {
    let workload = WorkloadId::new("apps.notes").expect("workload");
    let reference = DatabaseRef::parse(
        "fabric-resource-database-ref-v1:fabric-resource-database-deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
    )
    .expect("database ref");

    let duplicate_structured = vec![
        WorkloadBinding::new(
            workload.clone(),
            BindingName::new("DB").expect("binding name"),
            BindingTarget::database(reference.clone()),
        ),
        WorkloadBinding::new(
            workload.clone(),
            BindingName::new("DB").expect("binding name"),
            BindingTarget::database(reference.clone()),
        ),
    ];
    assert!(matches!(
        crate::validate_workload_bindings(&workload, &duplicate_structured, &[]),
        Err(WorkerError::InvalidInput { ref message })
            if message.contains("duplicate workload binding DB")
    ));

    let first = WorkloadBinding::new(
        workload.clone(),
        BindingName::new("DB_A").expect("binding name"),
        BindingTarget::database(reference.clone()),
    );
    let second = WorkloadBinding::new(
        workload.clone(),
        BindingName::new("DB_B").expect("binding name"),
        BindingTarget::database(reference),
    );
    let duplicate_environment = vec![
        WorkloadBindingProjection::environment(
            first.binding_id(),
            WorkloadBindingEnv::new("DATABASE_URL").expect("environment variable"),
        ),
        WorkloadBindingProjection::environment(
            second.binding_id(),
            WorkloadBindingEnv::new("DATABASE_URL").expect("environment variable"),
        ),
    ];
    assert!(matches!(
        crate::validate_workload_bindings(&workload, &[first, second], &duplicate_environment),
        Err(WorkerError::InvalidInput { ref message })
            if message.contains("duplicate environment binding projection for DATABASE_URL")
    ));
}

#[test]
fn secret_binding_keeps_authorization_context_without_plaintext_or_consumer_aliasing() {
    let workload = WorkloadId::new("apps.notes").expect("workload");
    let actor = ActorRef::new("workload.notes:runtime").expect("actor");
    let secret = SecretRef::new(
        SecretId::parse("notes-api-key").expect("secret id"),
        fabric_resource_authority::AuthorityScopeId::parse("scope.notes").expect("scope"),
    );
    let binding = WorkloadBinding::new(
        workload.clone(),
        BindingName::new("API_KEY").expect("binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(secret.clone(), actor.clone())),
    );
    let projection = WorkloadBindingProjection::environment(
        binding.binding_id(),
        WorkloadBindingEnv::new("NOTES_API_KEY").expect("environment variable"),
    );

    assert_eq!(binding.consumer(), &workload);
    assert!(matches!(
        binding.target(),
        BindingTarget::Secret(access)
            if access.secret() == &secret && access.actor() == &actor
    ));
    let debug = format!("{binding:?}");
    assert!(debug.contains("notes-api-key"));
    assert!(!debug.contains("top-secret"));
    assert!(matches!(
        projection.projection(),
        BindingProjection::Environment(variable) if variable.as_str() == "NOTES_API_KEY"
    ));
}

#[test]
fn secret_binding_projection_changes_do_not_change_secret_identity() {
    let workload = WorkloadId::new("apps.notes").expect("workload");
    let actor = ActorRef::new("workload.notes:runtime").expect("actor");
    let secret = SecretRef::new(
        SecretId::parse("notes-api-key").expect("secret id"),
        fabric_resource_authority::AuthorityScopeId::parse("scope.notes").expect("scope"),
    );
    let access = AuthorizedSecretRef::new(secret.clone(), actor);
    let binding = WorkloadBinding::new(
        workload.clone(),
        BindingName::new("API_KEY").expect("binding name"),
        BindingTarget::secret(access.clone()),
    );
    let first = WorkloadBindingProjection::environment(
        binding.binding_id(),
        WorkloadBindingEnv::new("NOTES_API_KEY_A").expect("environment variable"),
    );
    let second = WorkloadBindingProjection::environment(
        binding.binding_id(),
        WorkloadBindingEnv::new("NOTES_API_KEY_B").expect("environment variable"),
    );

    assert!(matches!(
        binding.target(),
        BindingTarget::Secret(reference) if reference.secret() == &secret
    ));
    assert!(matches!(
        first.projection(),
        BindingProjection::Environment(variable) if variable.as_str() == "NOTES_API_KEY_A"
    ));
    assert!(matches!(
        second.projection(),
        BindingProjection::Environment(variable) if variable.as_str() == "NOTES_API_KEY_B"
    ));
    assert_eq!(binding.consumer(), &workload);
}

#[test]
fn service_binding_keeps_stable_service_identity_distinct_from_consumer_and_projection() {
    let workload = WorkloadId::new("apps.notes").expect("workload");
    let service_id = ServiceId::parse("svc_notes_api").expect("service id");
    let binding = WorkloadBinding::new(
        workload.clone(),
        BindingName::new("NOTES_API").expect("binding name"),
        BindingTarget::service(service_id.clone()),
    );
    let structured = WorkloadBindingProjection::structured(binding.binding_id());
    let projected = WorkloadBindingProjection::environment(
        binding.binding_id(),
        WorkloadBindingEnv::new("NOTES_API_URL").expect("environment binding"),
    );

    assert_eq!(binding.consumer(), &workload);
    assert!(matches!(
        binding.target(),
        BindingTarget::Service(target) if target == &service_id
    ));
    assert!(matches!(
        structured.projection(),
        BindingProjection::Structured
    ));
    assert!(matches!(
        projected.projection(),
        BindingProjection::Environment(variable) if variable.as_str() == "NOTES_API_URL"
    ));
    assert_ne!(service_id.as_str(), workload.as_str());
}

fn test_worker_spec() -> WorkerSpec {
    test_worker_spec_with_workload("app:release")
}

fn test_worker_spec_with_workload(workload: &str) -> WorkerSpec {
    WorkerSpec {
        workload_id: WorkloadId::new(workload).expect("workload id"),
        requirement: WorkerRequirement {
            api_version: WorkerApiVersion::parse("1.0.0").expect("api version"),
            features: vec![
                WorkerFeature::new("js.module").expect("feature"),
                WorkerFeature::new("http.fetch").expect("feature"),
            ],
        },
        artifact: WorkloadArtifact {
            reference: "stel://artifact/test".to_owned(),
            sha256: [7; 32],
        },
        entrypoint: WorkloadEntrypoint::new("main.ts").expect("entrypoint"),
    }
}
