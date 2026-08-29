use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use fabric_adapter_ingress_pingora::{PingoraIngressAdapter, PingoraIngressConfig};
use fabric_component::{
    Component, ComponentId, ComponentRegistry, ComponentRuntime, ComponentRuntimeModule,
    ParticipationState, SurfaceId, SurfaceRegistry,
};
use fabric_component_gateway::{Gateway, GatewayModule};
use fabric_component_namespace::{Namespace, NamespaceModule, NamespaceName};
use fabric_component_publication::{PublicationContract, PublicationModule};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    module_factory,
};
use fabric_resource_connectivity::{
    ConnectivityContract, NativeConnectivity, NativeConnectivityConfig,
};
use fabric_resource_ingress::{IngressContract, LocalHttpIngressAccessContract, NativeIngress};
use fabric_resource_service::{
    MarkServiceTargetDrainingRequest, MarkServiceTargetReadyRequest, NativeServices,
    NativeServicesConfig, RegisterServiceTargetRequest, Service, ServiceContract,
    ServiceEndpointId, ServiceError, ServiceHttpRequest, ServiceHttpResponse,
    ServiceHttpTargetRuntime, ServiceHttpTargetRuntimeService, ServiceProtocol, ServiceRequirement,
    ServiceScope, ServiceTarget, ServiceTargetId, ServiceTargetState, WithdrawServiceTargetRequest,
};
use tempfile::TempDir;

use crate::{
    GatewayExposureContract, LocalGatewayExposureContract, LocalNameResolutionContract,
    ServiceMaterializationContract, ServiceMaterializationModule,
};

#[derive(Default)]
struct Capture {
    component_runtime: Mutex<Option<ComponentRuntime>>,
    component_registry: Mutex<Option<ComponentRegistry>>,
    gateway: Mutex<Option<Gateway>>,
    surfaces: Mutex<Option<SurfaceRegistry>>,
    namespace: Mutex<Option<Namespace>>,
    publications: Mutex<Option<PublicationContract>>,
    exposure: Mutex<Option<GatewayExposureContract>>,
    local_exposure: Mutex<Option<LocalGatewayExposureContract>>,
    local_names: Mutex<Option<LocalNameResolutionContract>>,
    materialization: Mutex<Option<ServiceMaterializationContract>>,
    service: Mutex<Option<ServiceContract>>,
    ingress: Mutex<Option<IngressContract>>,
    local_http_access: Mutex<Option<LocalHttpIngressAccessContract>>,
    connectivity: Mutex<Option<ConnectivityContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    component_runtime_requirement: ContractRequirement<ComponentRuntime>,
    component_registry_requirement: ContractRequirement<ComponentRegistry>,
    surface_requirement: ContractRequirement<SurfaceRegistry>,
    namespace_requirement: ContractRequirement<Namespace>,
    publication_requirement: ContractRequirement<PublicationContract>,
    gateway_requirement: ContractRequirement<Gateway>,
    exposure_requirement: ContractRequirement<GatewayExposureContract>,
    local_exposure_requirement: ContractRequirement<LocalGatewayExposureContract>,
    local_names_requirement: ContractRequirement<LocalNameResolutionContract>,
    materialization_requirement: ContractRequirement<ServiceMaterializationContract>,
    service_requirement: ContractRequirement<ServiceContract>,
    ingress_requirement: ContractRequirement<IngressContract>,
    local_http_access_requirement: ContractRequirement<LocalHttpIngressAccessContract>,
    connectivity_requirement: ContractRequirement<ConnectivityContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("test.component.service-materialization.capture")
                .expect("module id"),
            component_runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            component_registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            gateway_requirement: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            surface_requirement: ContractRequirement::provisional(
                fabric_component::surface_contract_id(),
            ),
            namespace_requirement: ContractRequirement::provisional(
                fabric_component_namespace::namespace_contract_id(),
            ),
            publication_requirement: ContractRequirement::provisional(
                fabric_component_publication::publication_contract_id(),
            ),
            exposure_requirement: ContractRequirement::provisional(
                crate::gateway_exposure_contract_id(),
            ),
            local_exposure_requirement: ContractRequirement::provisional(
                crate::local_gateway_exposure_contract_id(),
            ),
            local_names_requirement: ContractRequirement::provisional(
                crate::local_name_resolution_contract_id(),
            ),
            materialization_requirement: ContractRequirement::provisional(
                crate::service_materialization_contract_id(),
            ),
            service_requirement: ContractRequirement::provisional(
                fabric_resource_service::service_contract_id(),
            ),
            ingress_requirement: ContractRequirement::provisional(
                fabric_resource_ingress::ingress_contract_id(),
            ),
            local_http_access_requirement: ContractRequirement::provisional(
                fabric_resource_ingress::local_http_ingress_access_contract_id(),
            ),
            connectivity_requirement: ContractRequirement::provisional(
                fabric_resource_connectivity::connectivity_contract_id(),
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
            self.component_runtime_requirement.id().clone(),
            self.component_registry_requirement.id().clone(),
            self.gateway_requirement.id().clone(),
            self.surface_requirement.id().clone(),
            self.namespace_requirement.id().clone(),
            self.publication_requirement.id().clone(),
            self.exposure_requirement.id().clone(),
            self.local_exposure_requirement.id().clone(),
            self.local_names_requirement.id().clone(),
            self.materialization_requirement.id().clone(),
            self.service_requirement.id().clone(),
            self.ingress_requirement.id().clone(),
            self.local_http_access_requirement.id().clone(),
            self.connectivity_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        *self
            .capture
            .component_runtime
            .lock()
            .expect("component runtime capture") = Some(
            (*bindings
                .resolve(&self.component_runtime_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .component_registry
            .lock()
            .expect("component registry capture") = Some(
            (*bindings
                .resolve(&self.component_registry_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.capture.gateway.lock().expect("gateway capture") = Some(
            (*bindings
                .resolve(&self.gateway_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.capture.surfaces.lock().expect("surface capture") = Some(
            (*bindings
                .resolve(&self.surface_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.capture.namespace.lock().expect("namespace capture") = Some(
            (*bindings
                .resolve(&self.namespace_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .publications
            .lock()
            .expect("publication capture") = Some(
            (*bindings
                .resolve(&self.publication_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.capture.exposure.lock().expect("exposure capture") = Some(
            (*bindings
                .resolve(&self.exposure_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .local_exposure
            .lock()
            .expect("local exposure capture") = Some(
            (*bindings
                .resolve(&self.local_exposure_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .local_names
            .lock()
            .expect("local names capture") = Some(
            (*bindings
                .resolve(&self.local_names_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .materialization
            .lock()
            .expect("materialization capture") = Some(
            (*bindings
                .resolve(&self.materialization_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.capture.service.lock().expect("service capture") = Some(
            (*bindings
                .resolve(&self.service_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self.capture.ingress.lock().expect("ingress capture") = Some(
            (*bindings
                .resolve(&self.ingress_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .local_http_access
            .lock()
            .expect("local access capture") = Some(
            (*bindings
                .resolve(&self.local_http_access_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
        *self
            .capture
            .connectivity
            .lock()
            .expect("connectivity capture") = Some(
            (*bindings
                .resolve(&self.connectivity_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?)
            .clone(),
        );
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

struct LoopbackRuntime {
    address: SocketAddr,
}

struct FakeRuntime {
    status: u16,
    body: Vec<u8>,
}

pub(crate) struct LoopbackServer {
    thread: Option<thread::JoinHandle<()>>,
    pub(crate) address: SocketAddr,
}

pub(crate) struct MaterializationHarness {
    _tempdir: Option<TempDir>,
    pub(crate) instance: Instance,
    pub(crate) component_runtime: ComponentRuntime,
    pub(crate) component_registry: ComponentRegistry,
    pub(crate) gateway: Gateway,
    pub(crate) surfaces: SurfaceRegistry,
    pub(crate) namespace: Namespace,
    pub(crate) publications: PublicationContract,
    pub(crate) exposure: GatewayExposureContract,
    pub(crate) local_exposure: LocalGatewayExposureContract,
    pub(crate) local_names: LocalNameResolutionContract,
    pub(crate) materialization: ServiceMaterializationContract,
    pub(crate) service: ServiceContract,
    pub(crate) ingress: IngressContract,
    pub(crate) local_http_access: LocalHttpIngressAccessContract,
    pub(crate) connectivity: ConnectivityContract,
}

impl ServiceHttpTargetRuntimeService for LoopbackRuntime {
    fn dispatch_http(
        &self,
        _endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        request.validate()?;
        dispatch_to_loopback(self.address, request)
    }
}

impl ServiceHttpTargetRuntimeService for FakeRuntime {
    fn dispatch_http(
        &self,
        _endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        request.validate()?;
        Ok(ServiceHttpResponse {
            status: self.status,
            headers: vec![],
            body: self.body.clone(),
        })
    }
}

impl MaterializationHarness {
    pub(crate) fn new(instance_id: &str) -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        let root = tempdir.path().to_path_buf();
        Self::from_root(instance_id, &root, Some(tempdir))
    }

    fn from_root(instance_id: &str, root: &Path, tempdir: Option<TempDir>) -> Self {
        let capture = Arc::new(Capture::default());
        let block = BlockBuilder::new(
            BlockId::new("test.fabric.materialization".to_owned()).expect("block"),
        )
        .register_module(ComponentRuntimeModule::new())
        .register_module(NamespaceModule::new())
        .register_module(PublicationModule::new())
        .register_module(GatewayModule::new())
        .register_module(NativeServices::new(NativeServicesConfig {
            database_path: service_db_path(root),
        }))
        .register_module(module_factory(|| {
            NativeIngress::new(Arc::new(PingoraIngressAdapter::new(
                PingoraIngressConfig::new(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))),
            )))
        }))
        .register_module(NativeConnectivity::new(NativeConnectivityConfig {
            database_path: connectivity_db_path(root),
        }))
        .register_module(ServiceMaterializationModule::new())
        .register_module(CaptureModule::new(Arc::clone(&capture)))
        .build();
        let composition = CompositionBuilder::new(
            CompositionId::new("test.fabric.materialization").expect("composition"),
        )
        .register_block(block)
        .build()
        .expect("composition");
        let mut instance = composition
            .materialize(InstanceId::new(instance_id).expect("instance id"))
            .expect("materialize");
        instance.start().expect("start block");

        Self {
            _tempdir: tempdir,
            component_runtime: capture
                .component_runtime
                .lock()
                .expect("component runtime capture")
                .clone()
                .expect("component runtime contract"),
            component_registry: capture
                .component_registry
                .lock()
                .expect("component registry capture")
                .clone()
                .expect("component registry contract"),
            gateway: capture
                .gateway
                .lock()
                .expect("gateway capture")
                .clone()
                .expect("gateway contract"),
            surfaces: capture
                .surfaces
                .lock()
                .expect("surface capture")
                .clone()
                .expect("surface contract"),
            namespace: capture
                .namespace
                .lock()
                .expect("namespace capture")
                .clone()
                .expect("namespace contract"),
            publications: capture
                .publications
                .lock()
                .expect("publication capture")
                .clone()
                .expect("publication contract"),
            exposure: capture
                .exposure
                .lock()
                .expect("exposure capture")
                .clone()
                .expect("exposure contract"),
            local_exposure: capture
                .local_exposure
                .lock()
                .expect("local exposure capture")
                .clone()
                .expect("local exposure contract"),
            local_names: capture
                .local_names
                .lock()
                .expect("local names capture")
                .clone()
                .expect("local names contract"),
            materialization: capture
                .materialization
                .lock()
                .expect("materialization capture")
                .clone()
                .expect("materialization contract"),
            service: capture
                .service
                .lock()
                .expect("service capture")
                .clone()
                .expect("service contract"),
            ingress: capture
                .ingress
                .lock()
                .expect("ingress capture")
                .clone()
                .expect("ingress contract"),
            local_http_access: capture
                .local_http_access
                .lock()
                .expect("local access capture")
                .clone()
                .expect("local access contract"),
            connectivity: capture
                .connectivity
                .lock()
                .expect("connectivity capture")
                .clone()
                .expect("connectivity contract"),
            instance,
        }
    }

    pub(crate) fn component(&self, component_id: &str) -> Component {
        Component::bind(
            ComponentId::new(component_id).expect("component id"),
            &self.component_runtime,
        )
    }

    pub(crate) fn ensure_component_active(&self, component: &Component) {
        match self.component_registry.component(component.component_id()) {
            Ok(status)
                if status.participation().component() == component
                    && status.state() == ParticipationState::Active => {}
            Ok(status)
                if status.participation().component() == component
                    && status.state() == ParticipationState::Preparing =>
            {
                self.component_registry
                    .activate(status.participation())
                    .expect("activate existing component participation");
            }
            Ok(_) => panic!(
                "component `{}` is already registered under a different runtime identity",
                component.component_id()
            ),
            Err(fabric_component::ComponentError::UnknownComponent(_)) => {
                let participation = self
                    .component_registry
                    .register(component.clone(), Health::Healthy)
                    .expect("register component participation")
                    .participation()
                    .clone();
                self.component_registry
                    .activate(&participation)
                    .expect("activate component participation");
            }
            Err(error) => panic!("resolve component participation: {error}"),
        }
    }

    pub(crate) fn ensure_http_service(&self, scope: &str, name: &str) -> Service {
        self.service
            .ensure_service(
                &ServiceScope::new(scope).expect("scope"),
                &ServiceRequirement::new(ServiceProtocol::Http, name).expect("requirement"),
            )
            .expect("ensure service")
            .service
    }
}

impl Drop for MaterializationHarness {
    fn drop(&mut self) {
        self.instance.stop();
    }
}

impl Drop for LoopbackServer {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) fn register_surface(
    harness: &MaterializationHarness,
    component: &Component,
    surface_id: &str,
) -> fabric_component::Surface {
    harness
        .surfaces
        .register(
            component.clone(),
            SurfaceId::new(surface_id).expect("surface id"),
        )
        .expect("register surface")
}

pub(crate) fn publish_surface(
    harness: &MaterializationHarness,
    component: &Component,
    name: &str,
    surface: &fabric_component::Surface,
) -> fabric_component_publication::Publication {
    harness.ensure_component_active(component);
    let claim = harness
        .namespace
        .claim(
            component.clone(),
            NamespaceName::new(name).expect("namespace name"),
        )
        .expect("claim name");
    harness
        .publications
        .publish(claim, surface.clone())
        .expect("publish surface")
}

pub(crate) fn register_ready_loopback_target(
    harness: &MaterializationHarness,
    service: &Service,
    target_id: &str,
    server: &LoopbackServer,
) -> ServiceTargetId {
    let target = ServiceTarget {
        id: ServiceTargetId::new(target_id).expect("target id"),
        endpoint_id: service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(LoopbackRuntime {
                address: server.address,
            })),
        )
        .expect("register loopback target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("mark target ready");
    target.id
}

pub(crate) fn register_ready_target(
    harness: &MaterializationHarness,
    service: &Service,
    target_id: &str,
    status: u16,
    body: &'static str,
) -> ServiceTargetId {
    let target = ServiceTarget {
        id: ServiceTargetId::new(target_id).expect("target id"),
        endpoint_id: service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    harness
        .service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FakeRuntime {
                status,
                body: body.as_bytes().to_vec(),
            })),
        )
        .expect("register target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("mark target ready");
    target.id
}

pub(crate) fn replace_with_new_loopback_target(
    harness: &MaterializationHarness,
    service: &Service,
    previous_target_id: &ServiceTargetId,
    next_target_id: &str,
    next_server: &LoopbackServer,
) {
    harness
        .service
        .mark_target_draining(MarkServiceTargetDrainingRequest {
            service_id: service.id.clone(),
            target_id: previous_target_id.clone(),
        })
        .expect("drain previous target");
    harness
        .service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: service.id.clone(),
            target_id: previous_target_id.clone(),
        })
        .expect("withdraw previous target");
    let _ = register_ready_loopback_target(harness, service, next_target_id, next_server);
}

pub(crate) fn drain_and_withdraw_target(
    harness: &MaterializationHarness,
    service: &Service,
    target_id: &ServiceTargetId,
) {
    harness
        .service
        .mark_target_draining(MarkServiceTargetDrainingRequest {
            service_id: service.id.clone(),
            target_id: target_id.clone(),
        })
        .expect("drain target");
    harness
        .service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: service.id.clone(),
            target_id: target_id.clone(),
        })
        .expect("withdraw target");
}

pub(crate) fn spawn_loopback_server(status: u16, body: &'static str) -> LoopbackServer {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .expect("bind loopback server");
    let address = listener.local_addr().expect("loopback address");
    let body = body.as_bytes().to_vec();
    let thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept loopback request");
        let mut buffer = [0_u8; 4096];
        let _ = stream.read(&mut buffer);
        let response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response headers");
        stream.write_all(&body).expect("write response body");
        stream.flush().expect("flush response");
    });
    LoopbackServer {
        thread: Some(thread),
        address,
    }
}

fn service_db_path(root: &Path) -> PathBuf {
    root.join("services.sqlite")
}

fn connectivity_db_path(root: &Path) -> PathBuf {
    root.join("connectivity.sqlite")
}

fn dispatch_to_loopback(
    address: SocketAddr,
    request: ServiceHttpRequest,
) -> Result<ServiceHttpResponse, ServiceError> {
    let mut stream = TcpStream::connect(address).map_err(|error| ServiceError::DispatchFailed {
        message: format!("connect loopback upstream: {error}"),
    })?;
    let path = request_path_and_query(&request.url)?;
    let mut payload = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
        request.method,
        path,
        address,
        request.body.len()
    );
    for header in &request.headers {
        payload.push_str(&format!("{}: {}\r\n", header.name, header.value));
    }
    payload.push_str("\r\n");
    stream
        .write_all(payload.as_bytes())
        .and_then(|_| stream.write_all(&request.body))
        .and_then(|_| stream.flush())
        .map_err(|error| ServiceError::DispatchFailed {
            message: format!("write loopback upstream request: {error}"),
        })?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| ServiceError::DispatchFailed {
            message: format!("read loopback upstream response: {error}"),
        })?;
    parse_http_response(&response)
}

fn request_path_and_query(url: &str) -> Result<&str, ServiceError> {
    let Some((_, remainder)) = url.split_once("://") else {
        return Err(ServiceError::DispatchFailed {
            message: "loopback upstream request url is missing a scheme".to_owned(),
        });
    };
    Ok(match remainder.find('/') {
        Some(index) => &remainder[index..],
        None => "/",
    })
}

fn parse_http_response(bytes: &[u8]) -> Result<ServiceHttpResponse, ServiceError> {
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| ServiceError::DispatchFailed {
            message: "loopback upstream response is missing header terminator".to_owned(),
        })?;
    let (header_bytes, body_bytes) = bytes.split_at(header_end + 4);
    let header_text =
        std::str::from_utf8(header_bytes).map_err(|error| ServiceError::DispatchFailed {
            message: format!("loopback upstream response headers are not utf-8: {error}"),
        })?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().ok_or_else(|| ServiceError::DispatchFailed {
        message: "loopback upstream response is missing a status line".to_owned(),
    })?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| ServiceError::DispatchFailed {
            message: "loopback upstream response is missing a status code".to_owned(),
        })?
        .parse::<u16>()
        .map_err(|error| ServiceError::DispatchFailed {
            message: format!("loopback upstream response has an invalid status code: {error}"),
        })?;
    Ok(ServiceHttpResponse {
        status,
        headers: Vec::new(),
        body: body_bytes.to_vec(),
    })
}
