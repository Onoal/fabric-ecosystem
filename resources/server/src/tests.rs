use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    module_factory,
};
use fabric_resource_registry::{ResourceRegistry, ResourceRegistryModule};
use fabric_resource_service::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, ServiceContract, ServiceHttpRequest, ServiceHttpResponse,
    ServiceHttpTargetRuntime, ServiceHttpTargetRuntimeService, ServiceProtocol, ServiceRequirement,
    ServiceScope, ServiceTarget, ServiceTargetId, ServiceTargetState,
};
use tempfile::TempDir;

use crate::{
    DispatchServerHttpRequest, NativeServer, ServerAdapter, ServerContract, ServerError,
    ServerExecution, ServerExecutionRequest, ServerHttpContract, ServerHttpHeader,
    ServerHttpRequest, ServerHttpResponse as ResourceServerHttpResponse, ServerId,
    ServerInstanceId, ServerSpec, StartServerRequest, StopServerRequest, server_contract_id,
    server_http_contract_id, server_resource_id,
};

#[derive(Default)]
struct Capture {
    server: Mutex<Option<ServerContract>>,
    server_http: Mutex<Option<ServerHttpContract>>,
    registry: Mutex<Option<Arc<ResourceRegistry>>>,
    service: Mutex<Option<ServiceContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    server_requirement: ContractRequirement<ServerContract>,
    server_http_requirement: ContractRequirement<ServerHttpContract>,
    registry_requirement: ContractRequirement<ResourceRegistry>,
    service_requirement: ContractRequirement<ServiceContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.server.capture").expect("module id"),
            server_requirement: ContractRequirement::provisional(server_contract_id()),
            server_http_requirement: ContractRequirement::provisional(server_http_contract_id()),
            registry_requirement: ContractRequirement::provisional(
                fabric_resource_registry::resource_registry_contract_id(),
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
        vec![self.server_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.server_http_requirement.id().clone(),
            self.registry_requirement.id().clone(),
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
        let server = bindings
            .resolve(&self.server_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let server_http = bindings
            .resolve_optional(&self.server_http_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve_optional(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let service = bindings
            .resolve_optional(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.server.lock().expect("server capture") = Some((*server).clone());
        *self
            .capture
            .server_http
            .lock()
            .expect("server http capture") = server_http.as_deref().cloned();
        *self.capture.registry.lock().expect("registry capture") = registry;
        *self.capture.service.lock().expect("service capture") = service.as_deref().cloned();
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
    prepared: Vec<ServerSpec>,
    started: Vec<(ServerInstanceId, ServerSpec)>,
    dispatches: Vec<(ServerInstanceId, ServerHttpRequest)>,
    stopped: Vec<ServerInstanceId>,
    cleaned: usize,
}

#[derive(Clone)]
struct FakeServerAdapter {
    state: Arc<Mutex<AdapterState>>,
    supports_http_dispatch: bool,
}

impl ServerAdapter for FakeServerAdapter {
    fn supports_http_dispatch(&self) -> bool {
        self.supports_http_dispatch
    }

    fn prepare(&mut self, server: &ServerSpec) -> Result<(), ServerError> {
        self.state
            .lock()
            .expect("adapter state")
            .prepared
            .push(server.clone());
        Ok(())
    }

    fn start(
        &mut self,
        request: ServerExecutionRequest,
    ) -> Result<Box<dyn ServerExecution>, ServerError> {
        self.state
            .lock()
            .expect("adapter state")
            .started
            .push((request.instance_id.clone(), request.start.server.clone()));
        Ok(Box::new(FakeExecution {
            instance_id: request.instance_id,
            state: Arc::clone(&self.state),
            supports_http_dispatch: self.supports_http_dispatch,
        }))
    }

    fn clear(&mut self) {}
}

struct FakeExecution {
    instance_id: ServerInstanceId,
    state: Arc<Mutex<AdapterState>>,
    supports_http_dispatch: bool,
}

impl ServerExecution for FakeExecution {
    fn dispatch_http(
        &mut self,
        request: DispatchServerHttpRequest,
    ) -> Result<ResourceServerHttpResponse, ServerError> {
        if !self.supports_http_dispatch {
            return Err(ServerError::DispatchFailed {
                message: "fake server execution does not support http".to_owned(),
            });
        }
        request.request.validate()?;
        self.state
            .lock()
            .expect("adapter state")
            .dispatches
            .push((request.server_instance_id, request.request.clone()));
        Ok(ResourceServerHttpResponse {
            status: 200,
            headers: vec![ServerHttpHeader {
                name: "content-type".to_owned(),
                value: "text/plain".to_owned(),
            }],
            body: b"server-ok".to_vec(),
        })
    }

    fn stop(&mut self) -> Result<(), ServerError> {
        self.state
            .lock()
            .expect("adapter state")
            .stopped
            .push(self.instance_id.clone());
        Ok(())
    }

    fn cleanup(&mut self) {
        self.state.lock().expect("adapter state").cleaned += 1;
    }
}

struct ServerServiceTargetRuntime {
    server_http: ServerHttpContract,
    server_instance_id: ServerInstanceId,
    expected_url: String,
}

impl ServiceHttpTargetRuntimeService for ServerServiceTargetRuntime {
    fn dispatch_http(
        &self,
        _endpoint_id: &fabric_resource_service::ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, fabric_resource_service::ServiceError> {
        let response = self
            .server_http
            .dispatch_http(DispatchServerHttpRequest {
                server_instance_id: self.server_instance_id.clone(),
                request: ServerHttpRequest {
                    method: request.method,
                    url: request.url,
                    headers: request
                        .headers
                        .into_iter()
                        .map(|header| ServerHttpHeader {
                            name: header.name,
                            value: header.value,
                        })
                        .collect(),
                    body: request.body,
                },
            })
            .map_err(
                |error| fabric_resource_service::ServiceError::DispatchFailed {
                    message: error.to_string(),
                },
            )?;
        assert_eq!(self.expected_url, "http://server.local/health");
        Ok(ServiceHttpResponse {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(|header| fabric_resource_service::ServiceHttpHeader {
                    name: header.name,
                    value: header.value,
                })
                .collect(),
            body: response.body,
        })
    }
}

fn server_module(
    state: Arc<Mutex<AdapterState>>,
    supports_http_dispatch: bool,
) -> impl fabric_core::Module {
    module_factory(move || {
        NativeServer::with_adapter(Box::new(FakeServerAdapter {
            state: Arc::clone(&state),
            supports_http_dispatch,
        }))
    })
}

fn test_server_spec(id: &str) -> ServerSpec {
    ServerSpec {
        server_id: ServerId::new(id).expect("server id"),
        artifact: crate::ServerArtifact {
            reference: format!("fabric://artifact/{id}"),
            sha256: [7; 32],
        },
        entrypoint: crate::ServerEntrypoint::new("main.ts").expect("entrypoint"),
    }
}

fn start_instance(id: &str, composition: fabric_core::Composition) -> fabric_core::Instance {
    let mut instance = composition
        .materialize(InstanceId::new(id).expect("instance id"))
        .expect("materialize");
    instance.start().expect("start");
    instance
}

fn service_db_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join("services.sqlite")
}

fn target(id: &str, endpoint_id: &fabric_resource_service::ServiceEndpointId) -> ServiceTarget {
    ServiceTarget {
        id: ServiceTargetId::new(id).expect("service target id"),
        endpoint_id: endpoint_id.clone(),
        state: ServiceTargetState::Registered,
    }
}

#[test]
fn server_resource_is_open_and_http_facet_is_optional() {
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.server.resource".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("server".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
            .register_module(server_module(Arc::clone(&adapter_state), false))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = start_instance("fabric.server.resource", composition);

    let registry = capture
        .registry
        .lock()
        .expect("registry capture")
        .clone()
        .expect("captured registry");
    let resources = registry.resources();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].resource_id(), &server_resource_id());
    assert_eq!(
        resources[0].module_id().as_str(),
        "fabric.resource.server.native"
    );
    assert_eq!(
        resources[0]
            .provided_contracts()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![server_contract_id()]
    );
    assert!(
        capture
            .server_http
            .lock()
            .expect("server http capture")
            .is_none()
    );

    let server = capture
        .server
        .lock()
        .expect("server capture")
        .clone()
        .expect("captured server");
    let spec = test_server_spec("notes-server");
    let prepared = server.prepare_server(spec.clone()).expect("prepare");
    let live = server
        .start_server(StartServerRequest {
            prepared_server: prepared,
            server: spec.clone(),
        })
        .expect("start");
    assert_eq!(live.server_id, spec.server_id);
    server
        .stop_server(StopServerRequest {
            server_instance_id: live.server_instance_id,
        })
        .expect("stop");
    instance.stop();
}

#[test]
fn external_consumer_uses_only_server_contract() {
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.server.consumer".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("server".to_owned()).expect("block id"))
            .register_module(server_module(Arc::clone(&adapter_state), false))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = start_instance("fabric.server.consumer", composition);

    let server = capture
        .server
        .lock()
        .expect("server capture")
        .clone()
        .expect("captured server");
    let spec = test_server_spec("external-consumer");
    let prepared = server.prepare_server(spec.clone()).expect("prepare");
    let live = server
        .start_server(StartServerRequest {
            prepared_server: prepared,
            server: spec,
        })
        .expect("start");
    server
        .stop_server(StopServerRequest {
            server_instance_id: live.server_instance_id,
        })
        .expect("stop");
    instance.stop();
}

#[test]
fn fake_server_adapter_proves_server_resource_is_deno_free() {
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.server.fake-adapter".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("server".to_owned()).expect("block id"))
            .register_module(server_module(Arc::clone(&adapter_state), true))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = start_instance("fabric.server.fake-adapter", composition);

    let server = capture
        .server
        .lock()
        .expect("server capture")
        .clone()
        .expect("captured server");
    let server_http = capture
        .server_http
        .lock()
        .expect("server http capture")
        .clone()
        .expect("captured server http");
    let spec = test_server_spec("http-fake");
    let prepared = server.prepare_server(spec.clone()).expect("prepare");
    let live = server
        .start_server(StartServerRequest {
            prepared_server: prepared,
            server: spec,
        })
        .expect("start");
    let response = server_http
        .dispatch_http(DispatchServerHttpRequest {
            server_instance_id: live.server_instance_id.clone(),
            request: ServerHttpRequest {
                method: "GET".to_owned(),
                url: "http://server.local/health".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        })
        .expect("dispatch");
    assert_eq!(response.status, 200);
    server
        .stop_server(StopServerRequest {
            server_instance_id: live.server_instance_id.clone(),
        })
        .expect("stop");

    let state = adapter_state.lock().expect("adapter state");
    assert_eq!(state.prepared.len(), 1);
    assert_eq!(state.started.len(), 1);
    assert_eq!(state.dispatches.len(), 1);
    assert_eq!(state.stopped, vec![live.server_instance_id]);
    instance.stop();
}

#[test]
fn stable_service_id_survives_server_instance_replacement() {
    let tempdir = TempDir::new().expect("tempdir");
    let capture = Arc::new(Capture::default());
    let adapter_state = Arc::new(Mutex::new(AdapterState::default()));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.server.service-bridge".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("server".to_owned()).expect("block id"))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: service_db_path(tempdir.path()),
            }))
            .register_module(server_module(Arc::clone(&adapter_state), true))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = start_instance("fabric.server.service-bridge", composition);

    let server = capture
        .server
        .lock()
        .expect("server capture")
        .clone()
        .expect("captured server");
    let server_http = capture
        .server_http
        .lock()
        .expect("server http capture")
        .clone()
        .expect("captured server http");
    let services = capture
        .service
        .lock()
        .expect("service capture")
        .clone()
        .expect("captured services");

    let requirement =
        ServiceRequirement::new(ServiceProtocol::Http, "notes-api").expect("service requirement");
    let scope = ServiceScope::new("apps.notes.server").expect("service scope");
    let service = services
        .ensure_service(&scope, &requirement)
        .expect("ensure service")
        .service;

    let spec = test_server_spec("notes-http-server");
    let first = server
        .start_server(StartServerRequest {
            prepared_server: server.prepare_server(spec.clone()).expect("prepare"),
            server: spec.clone(),
        })
        .expect("start first");
    let first_target = target("target_a", &service.endpoint.id);
    services
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: first_target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(ServerServiceTargetRuntime {
                server_http: server_http.clone(),
                server_instance_id: first.server_instance_id.clone(),
                expected_url: "http://server.local/health".to_owned(),
            })),
        )
        .expect("register first target");
    services
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: first_target.id.clone(),
        })
        .expect("ready first target");

    let first_response = services
        .dispatch_http(
            &service.id,
            ServiceHttpRequest {
                method: "GET".to_owned(),
                url: "http://server.local/health".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        )
        .expect("dispatch first");
    assert_eq!(first_response.status, 200);

    server
        .stop_server(StopServerRequest {
            server_instance_id: first.server_instance_id.clone(),
        })
        .expect("stop first");
    services
        .withdraw_target(fabric_resource_service::WithdrawServiceTargetRequest {
            service_id: service.id.clone(),
            target_id: first_target.id.clone(),
        })
        .expect("withdraw first target");

    let second = server
        .start_server(StartServerRequest {
            prepared_server: server.prepare_server(spec.clone()).expect("prepare again"),
            server: spec,
        })
        .expect("start second");
    let second_target = target("target_b", &service.endpoint.id);
    services
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: second_target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(ServerServiceTargetRuntime {
                server_http,
                server_instance_id: second.server_instance_id.clone(),
                expected_url: "http://server.local/health".to_owned(),
            })),
        )
        .expect("register second target");
    services
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: second_target.id.clone(),
        })
        .expect("ready second target");

    let live = services
        .resolve_live_endpoint(&service.id)
        .expect("resolve live endpoint");
    assert_eq!(live.service_id, service.id);
    assert_ne!(first.server_instance_id, second.server_instance_id);

    let second_response = services
        .dispatch_http(
            &service.id,
            ServiceHttpRequest {
                method: "GET".to_owned(),
                url: "http://server.local/health".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        )
        .expect("dispatch second");
    assert_eq!(second_response.status, 200);

    instance.stop();
}
