use std::sync::Arc;

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, InstanceId,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime, module_factory,
};
use fabric_resource_service::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, ServiceContract, ServiceError, ServiceHttpRequest,
    ServiceHttpResponse, ServiceHttpTargetRuntime, ServiceHttpTargetRuntimeService,
    ServiceProtocol, ServiceRequirement, ServiceScope, ServiceTarget, ServiceTargetId,
    ServiceTargetState,
};
use fabric_resource_worker::{
    BindingName, BindingTarget, DispatchHttpRequest, HttpRequest, NativeWorker,
    NativeWorkloadProjection, StartWorkerRequest, WorkerAdapter, WorkerApiVersion,
    WorkerCapabilities, WorkerContract, WorkerError, WorkerExecution, WorkerExecutionRequest,
    WorkerFeature, WorkerFeatureSupport, WorkerHttpContract, WorkerRequirement, WorkerSpec,
    WorkloadArtifact, WorkloadBinding, WorkloadBindingEnv, WorkloadBindingProjection,
    WorkloadEntrypoint, WorkloadId,
};
use sha2::{Digest, Sha256};

use crate::adapter::DenoWorkerAdapter;
use crate::artifact::{DenoArtifactResolver, ResolvedDenoArtifact};
use crate::config::DenoWorkerConfig;

struct MissingResolver;

impl DenoArtifactResolver for MissingResolver {
    fn resolve(
        &self,
        _artifact: &WorkloadArtifact,
    ) -> Result<ResolvedDenoArtifact, fabric_resource_worker::WorkerError> {
        Err(fabric_resource_worker::WorkerError::PrepareFailed {
            message: "resolver not configured".to_owned(),
        })
    }
}

#[derive(Default)]
struct Capture {
    worker: std::sync::Mutex<Option<WorkerContract>>,
    worker_http: std::sync::Mutex<Option<WorkerHttpContract>>,
    service: std::sync::Mutex<Option<ServiceContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    requirement: ContractRequirement<WorkerContract>,
    http_requirement: ContractRequirement<WorkerHttpContract>,
    service_requirement: ContractRequirement<ServiceContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.worker.deno.capture").expect("module id"),
            requirement: ContractRequirement::provisional(
                fabric_resource_worker::worker_contract_id(),
            ),
            http_requirement: ContractRequirement::provisional(
                fabric_resource_worker::worker_http_contract_id(),
            ),
            service_requirement: ContractRequirement::provisional(
                fabric_resource_service::service_contract_id(),
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
        vec![self.requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.http_requirement.id().clone(),
            self.service_requirement.id().clone(),
        ]
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
        let worker_http = bindings
            .resolve_optional(&self.http_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let service = bindings
            .resolve_optional(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.worker.lock().expect("capture lock") = Some((*worker).clone());
        *self.capture.worker_http.lock().expect("http capture lock") =
            worker_http.as_deref().cloned();
        *self.capture.service.lock().expect("service capture lock") = service.as_deref().cloned();
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

struct FixedResponseRuntime {
    expected_method: String,
    expected_url: String,
    body: String,
}

impl ServiceHttpTargetRuntimeService for FixedResponseRuntime {
    fn dispatch_http(
        &self,
        _endpoint_id: &fabric_resource_service::ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        assert_eq!(request.method, self.expected_method);
        assert_eq!(request.url, self.expected_url);
        Ok(ServiceHttpResponse {
            status: 200,
            headers: Vec::new(),
            body: self.body.clone().into_bytes(),
        })
    }
}

struct TestWorkerAdapter;

impl WorkerAdapter for TestWorkerAdapter {
    fn capabilities(&self) -> WorkerCapabilities {
        WorkerCapabilities {
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

fn deno_worker_module(
    config: DenoWorkerConfig,
    resolver: Arc<dyn DenoArtifactResolver>,
) -> impl fabric_core::Module {
    module_factory(move || {
        NativeWorker::with_adapter(Box::new(
            DenoWorkerAdapter::new(config.clone(), Arc::clone(&resolver)).expect("adapter"),
        ))
    })
}

#[test]
fn normal_suite_does_not_require_deno_bin() {
    let config = DenoWorkerConfig::new(
        std::path::PathBuf::from("/nonexistent/deno"),
        tempfile::tempdir().expect("tempdir").path().join("runtime"),
    );
    let _adapter = DenoWorkerAdapter::new(config, Arc::new(MissingResolver)).expect("adapter");
}

#[test]
fn deno_adapter_rejects_non_js_requirement_without_runtime_catalog_changes() {
    let config = DenoWorkerConfig::new(
        std::path::PathBuf::from("/nonexistent/deno"),
        tempfile::tempdir().expect("tempdir").path().join("runtime"),
    );
    let resolver: Arc<dyn DenoArtifactResolver> = Arc::new(MissingResolver);
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.deno.non-js-incompatible".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(deno_worker_module(config, resolver))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut composition = composition
        .materialize(
            InstanceId::new("fabric.resource.worker.deno.non-js-incompatible".to_owned())
                .expect("instance id"),
        )
        .expect("materialize composition");
    composition.start().expect("start composition");
    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");

    let error = worker
        .prepare_worker(WorkerSpec {
            workload_id: WorkloadId::new("non-js-only").expect("workload id"),
            requirement: WorkerRequirement {
                api_version: WorkerApiVersion::parse("1.0.0").expect("api version"),
                features: vec![WorkerFeature::new("python.module").expect("feature")],
            },
            artifact: WorkloadArtifact {
                reference: "fabric://artifact/non-js-only".to_owned(),
                sha256: [3; 32],
            },
            entrypoint: WorkloadEntrypoint::new("run.sh").expect("entrypoint"),
        })
        .expect_err("deno should reject non-js workloads");
    assert!(matches!(
        error,
        WorkerError::Incompatible { ref issues } if !issues.is_empty()
    ));
}

#[test]
fn worker_provider_can_be_composed_without_apps() {
    let capture = Arc::new(Capture::default());
    let block =
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(module_factory(|| {
                NativeWorker::with_adapter(Box::new(TestWorkerAdapter))
            }))
            .build();
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.donor".to_owned()).expect("composition id"),
    )
    .register_block(block)
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut composition = composition
        .materialize(
            InstanceId::new("fabric.resource.worker.donor".to_owned()).expect("instance id"),
        )
        .expect("materialize composition");
    composition.start().expect("start");
    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let spec = test_worker_spec();
    let prepared = worker.prepare_worker(spec.clone()).expect("prepare");
    let instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: spec.clone(),
            bindings: Vec::new(),
            binding_projections: Vec::new(),
        })
        .expect("start");
    assert_eq!(instance.workload_id, spec.workload_id);
}

#[test]
#[ignore = "requires DENO_BIN=/path/to/deno 2.9.5"]
fn real_deno_executes_zero_binding_workload() {
    let deno_bin = std::env::var("DENO_BIN").expect("DENO_BIN");
    let root = tempfile::tempdir().expect("tempdir");
    let module_root = root.path().join("module");
    std::fs::create_dir_all(&module_root).expect("module root");
    let entry = module_root.join("main.ts");
    std::fs::write(
        &entry,
        r#"
export async function fetch(_request, _env) {
  return new Response("hello-deno", { status: 200 });
}
"#,
    )
    .expect("write entry");
    let artifact_file = root.path().join("artifact.bin");
    std::fs::write(&artifact_file, b"deno-artifact").expect("artifact");
    let digest = Sha256::digest(std::fs::read(&artifact_file).expect("read artifact"));
    let artifact = WorkloadArtifact {
        reference: "stel://deno/test".to_owned(),
        sha256: digest.into(),
    };

    #[derive(Clone)]
    struct Resolver {
        artifact_file: std::path::PathBuf,
        module_root: std::path::PathBuf,
    }

    impl DenoArtifactResolver for Resolver {
        fn resolve(
            &self,
            _artifact: &WorkloadArtifact,
        ) -> Result<ResolvedDenoArtifact, fabric_resource_worker::WorkerError> {
            Ok(ResolvedDenoArtifact {
                artifact_file: self.artifact_file.clone(),
                module_root: self.module_root.clone(),
            })
        }
    }

    let config = DenoWorkerConfig::new(
        std::path::PathBuf::from(deno_bin),
        root.path().join("runtime"),
    );
    let resolver: Arc<dyn DenoArtifactResolver> = Arc::new(Resolver {
        artifact_file: artifact_file.clone(),
        module_root: module_root.clone(),
    });
    let block =
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(deno_worker_module(config, resolver))
            .build();
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.deno.real".to_owned()).expect("composition id"),
    )
    .register_block(block)
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut composition = composition
        .materialize(
            InstanceId::new("fabric.resource.worker.deno.real".to_owned()).expect("instance id"),
        )
        .expect("materialize composition");
    composition.start().expect("start composition");
    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let spec = WorkerSpec {
        workload_id: WorkloadId::new("real-deno").expect("workload id"),
        requirement: WorkerRequirement {
            api_version: WorkerApiVersion::parse("1.0.0").expect("api version"),
            features: vec![
                WorkerFeature::new("js.module").expect("feature"),
                WorkerFeature::new("http.fetch").expect("feature"),
            ],
        },
        artifact,
        entrypoint: WorkloadEntrypoint::new("main.ts").expect("entrypoint"),
    };
    let prepared = worker.prepare_worker(spec.clone()).expect("prepare");
    let instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: spec,
            bindings: Vec::new(),
            binding_projections: Vec::new(),
        })
        .expect("start");
    assert_eq!(
        instance.status,
        fabric_resource_worker::WorkerInstanceStatus::Running
    );
}

#[test]
#[ignore = "requires DENO_BIN=/path/to/deno 2.9.5 and unrestricted loopback listeners"]
fn real_deno_consumes_projected_service_binding() {
    let deno_bin = std::env::var("DENO_BIN").expect("DENO_BIN");
    let root = tempfile::tempdir().expect("tempdir");
    let module_root = root.path().join("module");
    std::fs::create_dir_all(&module_root).expect("module root");
    let entry = module_root.join("main.ts");
    std::fs::write(
        &entry,
        r#"
export async function fetch(request, _env) {
  const pathname = new URL(request.url).pathname;
  if (pathname !== "/service") {
    return new Response("not-found", { status: 404 });
  }
  const baseUrl = Deno.env.get("SERVICE_BASE_URL");
  if (!baseUrl) {
    return new Response("missing-service-base-url", { status: 500 });
  }
  const response = await globalThis.fetch(`${baseUrl}/status`);
  const body = await response.text();
  return new Response(body, { status: response.status });
}
"#,
    )
    .expect("write entry");
    let artifact_file = root.path().join("artifact.bin");
    std::fs::write(&artifact_file, b"deno-service-artifact").expect("artifact");
    let digest = Sha256::digest(std::fs::read(&artifact_file).expect("read artifact"));
    let artifact = WorkloadArtifact {
        reference: "stel://deno/service-test".to_owned(),
        sha256: digest.into(),
    };

    #[derive(Clone)]
    struct Resolver {
        artifact_file: std::path::PathBuf,
        module_root: std::path::PathBuf,
    }

    impl DenoArtifactResolver for Resolver {
        fn resolve(
            &self,
            _artifact: &WorkloadArtifact,
        ) -> Result<ResolvedDenoArtifact, fabric_resource_worker::WorkerError> {
            Ok(ResolvedDenoArtifact {
                artifact_file: self.artifact_file.clone(),
                module_root: self.module_root.clone(),
            })
        }
    }

    let config = DenoWorkerConfig::new(
        std::path::PathBuf::from(deno_bin),
        root.path().join("runtime"),
    );
    let resolver: Arc<dyn DenoArtifactResolver> = Arc::new(Resolver {
        artifact_file: artifact_file.clone(),
        module_root: module_root.clone(),
    });
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.deno.service".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("block id"))
            .register_module(deno_worker_module(config, resolver))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.projection".to_owned()).expect("block id"))
            .register_module(NativeWorkloadProjection::new())
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("fabric.resource.service".to_owned()).expect("block id"))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: root.path().join("services.sqlite"),
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
    let mut composition = composition
        .materialize(
            InstanceId::new("fabric.resource.worker.deno.service".to_owned()).expect("instance id"),
        )
        .expect("materialize composition");
    composition.start().expect("start composition");

    let worker = capture
        .worker
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured worker");
    let worker_http = capture
        .worker_http
        .lock()
        .expect("http capture lock")
        .clone()
        .expect("captured worker http");
    let service = capture
        .service
        .lock()
        .expect("service capture lock")
        .clone()
        .expect("captured service");
    let service_model = service
        .ensure_service(
            &ServiceScope::new("apps.notes.api").expect("scope"),
            &ServiceRequirement::new(ServiceProtocol::Http, "api").expect("requirement"),
        )
        .expect("ensure service")
        .service;
    let target = ServiceTarget {
        id: ServiceTargetId::new("notes_api_a").expect("target id"),
        endpoint_id: service_model.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service_model.id.clone(),
                target: target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FixedResponseRuntime {
                expected_method: "GET".to_owned(),
                expected_url: "/status".to_owned(),
                body: "service-deno-ok".to_owned(),
            })),
        )
        .expect("register target");
    service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service_model.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("mark target ready");

    let spec = WorkerSpec {
        workload_id: WorkloadId::new("real-deno-service").expect("workload id"),
        requirement: WorkerRequirement {
            api_version: WorkerApiVersion::parse("1.0.0").expect("api version"),
            features: vec![
                WorkerFeature::new("js.module").expect("feature"),
                WorkerFeature::new("http.fetch").expect("feature"),
            ],
        },
        artifact,
        entrypoint: WorkloadEntrypoint::new("main.ts").expect("entrypoint"),
    };
    let prepared = worker.prepare_worker(spec.clone()).expect("prepare");
    let binding = WorkloadBinding::new(
        spec.workload_id.clone(),
        BindingName::new("service").expect("binding name"),
        BindingTarget::service(service_model.id.clone()),
    );
    let instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker: prepared,
            worker: spec,
            bindings: vec![binding.clone()],
            binding_projections: vec![WorkloadBindingProjection::environment(
                binding.binding_id(),
                WorkloadBindingEnv::new("SERVICE_BASE_URL").expect("env"),
            )],
        })
        .expect("start");

    let response = worker_http
        .dispatch_http(DispatchHttpRequest {
            worker_instance_id: instance.worker_instance_id.clone(),
            request: HttpRequest {
                method: "GET".to_owned(),
                url: "/service".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        })
        .expect("dispatch worker http");
    assert_eq!(response.status, 200);
    assert_eq!(
        String::from_utf8(response.body).expect("response body"),
        "service-deno-ok"
    );
}

fn test_worker_spec() -> WorkerSpec {
    WorkerSpec {
        workload_id: WorkloadId::new("zero-binding").expect("workload id"),
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
