use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};
use tempfile::TempDir;

use crate::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, ServiceContract, ServiceEndpointId, ServiceError,
    ServiceHttpRequest, ServiceHttpResponse, ServiceHttpTargetRuntime,
    ServiceHttpTargetRuntimeService, ServiceProtocol, ServiceRequirement, ServiceScope,
    ServiceTarget, ServiceTargetId, ServiceTargetState, WithdrawServiceTargetRequest,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedDispatch {
    endpoint_id: ServiceEndpointId,
    request: ServiceHttpRequest,
}

#[derive(Default)]
struct RuntimeProbe {
    requests: Mutex<Vec<RecordedDispatch>>,
}

struct FakeRuntime {
    probe: Arc<RuntimeProbe>,
}

impl ServiceHttpTargetRuntimeService for FakeRuntime {
    fn dispatch_http(
        &self,
        endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        request.validate()?;
        self.probe
            .requests
            .lock()
            .expect("probe")
            .push(RecordedDispatch {
                endpoint_id: endpoint_id.clone(),
                request,
            });
        Ok(ServiceHttpResponse {
            status: 200,
            headers: Vec::new(),
            body: b"service-ok".to_vec(),
        })
    }
}

#[derive(Default)]
struct ServiceCapture {
    contract: Mutex<Option<ServiceContract>>,
}

#[derive(Clone)]
struct ServiceCaptureModule {
    module_id: ModuleId,
    requirement: ContractRequirement<ServiceContract>,
    capture: Arc<ServiceCapture>,
}

impl ServiceCaptureModule {
    fn new(capture: Arc<ServiceCapture>) -> Self {
        Self {
            module_id: ModuleId::new("test.service.capture").expect("module"),
            requirement: ContractRequirement::provisional(crate::service_contract_id()),
            capture,
        }
    }
}

impl ModuleRuntime for ServiceCaptureModule {
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
        let service = bindings
            .resolve(&self.requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.contract.lock().expect("capture") = Some((*service).clone());
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

struct ServiceHarness {
    _tempdir: TempDir,
    instance: Instance,
    service: ServiceContract,
    probe: Arc<RuntimeProbe>,
}

impl ServiceHarness {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        let capture = Arc::new(ServiceCapture::default());
        let probe = Arc::new(RuntimeProbe::default());
        let block =
            BlockBuilder::new(BlockId::new("test.service.block".to_owned()).expect("block"))
                .register_module(NativeServices::new(NativeServicesConfig {
                    database_path: service_db_path(tempdir.path()),
                }))
                .register_module(ServiceCaptureModule::new(Arc::clone(&capture)))
                .build();
        let composition =
            CompositionBuilder::new(CompositionId::new("test.service").expect("composition"))
                .register_block(block)
                .build()
                .expect("composition");
        let mut instance = composition
            .materialize(InstanceId::new("test.service").expect("instance id"))
            .expect("materialize");
        instance.start().expect("start");
        let service = capture
            .contract
            .lock()
            .expect("capture")
            .clone()
            .expect("service contract");
        Self {
            _tempdir: tempdir,
            instance,
            service,
            probe,
        }
    }
}

impl Drop for ServiceHarness {
    fn drop(&mut self) {
        self.instance.stop();
    }
}

fn service_db_path(root: &std::path::Path) -> PathBuf {
    root.join("services.sqlite")
}

fn requirement() -> ServiceRequirement {
    ServiceRequirement::new(ServiceProtocol::Http, "web").expect("service requirement")
}

fn scope() -> ServiceScope {
    ServiceScope::new("apps.notes.web").expect("scope")
}

fn target_for(service: &crate::Service, suffix: &str) -> ServiceTarget {
    ServiceTarget {
        id: ServiceTargetId::new(format!("target_{suffix}")).expect("service target id"),
        endpoint_id: service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    }
}

fn fake_runtime(probe: &Arc<RuntimeProbe>) -> ServiceHttpTargetRuntime {
    ServiceHttpTargetRuntime::new(Arc::new(FakeRuntime {
        probe: Arc::clone(probe),
    }))
}

#[test]
fn service_ids_are_stable_for_scope_and_requirement() {
    let harness = ServiceHarness::new();
    let prepared = harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service");
    let resolved = harness
        .service
        .resolve_service(&scope(), &requirement())
        .expect("resolve service");

    assert_eq!(prepared.service.id, resolved.id);
    assert_eq!(prepared.service.endpoint, resolved.endpoint);
    assert!(!prepared.service.id.as_str().is_empty());
}

#[test]
fn service_target_registration_and_withdrawal_are_runtime_state() {
    let harness = ServiceHarness::new();
    let service = harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service")
        .service;
    let first = target_for(&service, "a");
    let second = target_for(&service, "b");

    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: first.clone(),
            },
            fake_runtime(&harness.probe),
        )
        .expect("register first target");
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: second.clone(),
            },
            fake_runtime(&harness.probe),
        )
        .expect("register second target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: second.id.clone(),
        })
        .expect("ready second target");

    let targets = harness
        .service
        .list_targets(&service.id)
        .expect("list targets");
    assert_eq!(
        targets,
        vec![
            first.clone(),
            ServiceTarget {
                state: ServiceTargetState::Ready,
                ..second.clone()
            },
        ]
    );

    harness
        .service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: service.id.clone(),
            target_id: first.id.clone(),
        })
        .expect("withdraw target");

    let remaining = harness
        .service
        .list_targets(&service.id)
        .expect("list targets after withdraw");
    assert_eq!(
        remaining,
        vec![ServiceTarget {
            state: ServiceTargetState::Ready,
            ..second
        }]
    );
}

#[test]
fn ready_service_resolves_current_live_endpoint() {
    let harness = ServiceHarness::new();
    let service = harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service")
        .service;
    let target = target_for(&service, "live-endpoint");
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: target.clone(),
            },
            fake_runtime(&harness.probe),
        )
        .expect("register target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("ready target");

    let live_endpoint = harness
        .service
        .resolve_live_endpoint(&service.id)
        .expect("resolve live endpoint");

    assert_eq!(live_endpoint.service_id, service.id);
    assert_eq!(live_endpoint.target_id, target.id);
    assert_eq!(live_endpoint.protocol, ServiceProtocol::Http);
}

#[test]
fn dispatch_http_requires_a_ready_target() {
    let harness = ServiceHarness::new();
    let service = harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service")
        .service;
    let target = target_for(&service, "registered-only");
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target,
            },
            fake_runtime(&harness.probe),
        )
        .expect("register target");

    let error = harness
        .service
        .dispatch_http(
            &service.id,
            ServiceHttpRequest {
                method: "GET".to_owned(),
                url: "http://service.local/notes".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        )
        .expect_err("dispatch should fail");

    assert_eq!(error, ServiceError::NoTarget);
}

#[test]
fn service_dispatches_http_without_apps_or_worker() {
    let harness = ServiceHarness::new();
    let service = harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service")
        .service;
    let target = target_for(&service, "dispatch");
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: target.clone(),
            },
            fake_runtime(&harness.probe),
        )
        .expect("register target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("ready target");

    let response = harness
        .service
        .dispatch_http(
            &service.id,
            ServiceHttpRequest {
                method: "POST".to_owned(),
                url: "http://service.local/proof?x=1".to_owned(),
                headers: Vec::new(),
                body: b"payload".to_vec(),
            },
        )
        .expect("dispatch");

    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"service-ok".to_vec());

    let requests = harness.probe.requests.lock().expect("probe");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].endpoint_id, service.endpoint.id);
    assert_eq!(requests[0].request.method, "POST");
    assert_eq!(requests[0].request.url, "http://service.local/proof?x=1");
}

#[test]
fn replacing_ready_target_updates_current_live_endpoint_without_changing_service_id() {
    let harness = ServiceHarness::new();
    let service = harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service")
        .service;
    let first = target_for(&service, "first");
    let second = target_for(&service, "second");

    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: first.clone(),
            },
            fake_runtime(&harness.probe),
        )
        .expect("register first target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: first.id.clone(),
        })
        .expect("ready first target");
    let first_endpoint = harness
        .service
        .resolve_live_endpoint(&service.id)
        .expect("resolve first endpoint");

    harness
        .service
        .mark_target_draining(crate::MarkServiceTargetDrainingRequest {
            service_id: service.id.clone(),
            target_id: first.id.clone(),
        })
        .expect("drain first target");
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: second.clone(),
            },
            fake_runtime(&harness.probe),
        )
        .expect("register second target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: second.id.clone(),
        })
        .expect("ready second target");

    let second_endpoint = harness
        .service
        .resolve_live_endpoint(&service.id)
        .expect("resolve second endpoint");

    assert_eq!(first_endpoint.service_id, service.id);
    assert_eq!(second_endpoint.service_id, service.id);
    assert_ne!(first_endpoint.target_id, second_endpoint.target_id);
}

#[test]
fn restart_preserves_service_identity_but_clears_live_targets() {
    let tempdir = TempDir::new().expect("tempdir");
    let database_path = service_db_path(tempdir.path());
    let service_id = {
        let capture = Arc::new(ServiceCapture::default());
        let probe = Arc::new(RuntimeProbe::default());
        let block =
            BlockBuilder::new(BlockId::new("test.service.restart.a".to_owned()).expect("block"))
                .register_module(NativeServices::new(NativeServicesConfig {
                    database_path: database_path.clone(),
                }))
                .register_module(ServiceCaptureModule::new(Arc::clone(&capture)))
                .build();
        let composition = CompositionBuilder::new(
            CompositionId::new("test.service.restart.a").expect("composition"),
        )
        .register_block(block)
        .build()
        .expect("composition");
        let mut instance = composition
            .materialize(InstanceId::new("test.service.restart").expect("instance id"))
            .expect("materialize");
        instance.start().expect("start");
        let service = capture
            .contract
            .lock()
            .expect("capture")
            .clone()
            .expect("service contract");
        let ensured = service
            .ensure_service(&scope(), &requirement())
            .expect("ensure service")
            .service;
        let target = target_for(&ensured, "restart");
        service
            .register_target(
                RegisterServiceTargetRequest {
                    service_id: ensured.id.clone(),
                    target: target.clone(),
                },
                fake_runtime(&probe),
            )
            .expect("register target");
        service
            .mark_target_ready(MarkServiceTargetReadyRequest {
                service_id: ensured.id.clone(),
                target_id: target.id,
            })
            .expect("ready target");
        let listed = service.list_targets(&ensured.id).expect("list targets");
        assert_eq!(listed.len(), 1);
        let service_id = ensured.id.clone();
        instance.stop();
        service_id
    };

    let capture = Arc::new(ServiceCapture::default());
    let block =
        BlockBuilder::new(BlockId::new("test.service.restart.b".to_owned()).expect("block"))
            .register_module(NativeServices::new(NativeServicesConfig { database_path }))
            .register_module(ServiceCaptureModule::new(Arc::clone(&capture)))
            .build();
    let composition =
        CompositionBuilder::new(CompositionId::new("test.service.restart.b").expect("composition"))
            .register_block(block)
            .build()
            .expect("composition");
    let mut instance = composition
        .materialize(InstanceId::new("test.service.restart").expect("instance id"))
        .expect("materialize");
    instance.start().expect("restart");
    let service = capture
        .contract
        .lock()
        .expect("capture")
        .clone()
        .expect("service contract");
    let resolved = service
        .resolve_service(&scope(), &requirement())
        .expect("resolve service");
    assert_eq!(resolved.id, service_id);
    assert!(
        service
            .list_targets(&resolved.id)
            .expect("list targets")
            .is_empty()
    );
}
