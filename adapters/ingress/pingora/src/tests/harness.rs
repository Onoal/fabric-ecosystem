use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    module_factory,
};
use fabric_resource_ingress::{
    IngressContract, IngressRouteTarget, LocalHttpIngressAccessContract, NativeIngress,
};
use fabric_resource_service::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, ServiceContract, ServiceEndpointId, ServiceError,
    ServiceHttpRequest, ServiceHttpResponse, ServiceHttpTargetRuntime,
    ServiceHttpTargetRuntimeService, ServiceProtocol, ServiceRequirement, ServiceScope,
    ServiceTarget, ServiceTargetId, ServiceTargetState,
};
use tempfile::TempDir;

use crate::{PingoraIngressAdapter, PingoraIngressConfig};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecordedDispatch {
    pub(crate) endpoint_id: ServiceEndpointId,
    pub(crate) request: ServiceHttpRequest,
}

#[derive(Default)]
pub(crate) struct RuntimeProbe {
    pub(crate) requests: Mutex<Vec<RecordedDispatch>>,
}

struct FakeRuntime {
    probe: Arc<RuntimeProbe>,
    status: u16,
    body: Vec<u8>,
}

struct LoopbackRuntime {
    address: SocketAddr,
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
            status: self.status,
            headers: vec![
                fabric_resource_service::ServiceHttpHeader::new("content-type", "text/plain")
                    .expect("header"),
            ],
            body: self.body.clone(),
        })
    }
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

#[derive(Default)]
struct Capture {
    service: Mutex<Option<ServiceContract>>,
    ingress: Mutex<Option<IngressContract>>,
    local_http_access: Mutex<Option<LocalHttpIngressAccessContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    service_requirement: ContractRequirement<ServiceContract>,
    ingress_requirement: ContractRequirement<IngressContract>,
    local_http_access_requirement: ContractRequirement<LocalHttpIngressAccessContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("test.pingora.capture").expect("module"),
            service_requirement: ContractRequirement::provisional(
                fabric_resource_service::service_contract_id(),
            ),
            ingress_requirement: ContractRequirement::provisional(
                fabric_resource_ingress::ingress_contract_id(),
            ),
            local_http_access_requirement: ContractRequirement::provisional(
                fabric_resource_ingress::local_http_ingress_access_contract_id(),
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
            self.service_requirement.id().clone(),
            self.ingress_requirement.id().clone(),
            self.local_http_access_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let service = bindings
            .resolve(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let ingress = bindings
            .resolve(&self.ingress_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let local_http_access = bindings
            .resolve(&self.local_http_access_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.service.lock().expect("service capture") = Some((*service).clone());
        *self.capture.ingress.lock().expect("ingress capture") = Some((*ingress).clone());
        *self
            .capture
            .local_http_access
            .lock()
            .expect("local ingress capture") = Some((*local_http_access).clone());
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

pub(crate) struct PingoraHarness {
    _tempdir: Option<TempDir>,
    instance: Instance,
    pub(crate) service: ServiceContract,
    pub(crate) ingress: IngressContract,
    pub(crate) local_http_access: LocalHttpIngressAccessContract,
    pub(crate) probe: Arc<RuntimeProbe>,
}

pub(crate) struct LoopbackServer {
    thread: Option<thread::JoinHandle<()>>,
    pub(crate) address: SocketAddr,
}

impl PingoraHarness {
    pub(crate) fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        let root = tempdir.path().to_path_buf();
        Self::from_root(&root, Some(tempdir))
    }

    pub(crate) fn from_existing_root(root: &Path) -> Self {
        Self::from_root(root, None)
    }

    fn from_root(root: &Path, tempdir: Option<TempDir>) -> Self {
        let probe = Arc::new(RuntimeProbe::default());
        let capture = Arc::new(Capture::default());
        let bind_address = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let block =
            BlockBuilder::new(BlockId::new("test.pingora.block".to_owned()).expect("block"))
                .register_module(NativeServices::new(NativeServicesConfig {
                    database_path: service_db_path(root),
                }))
                .register_module(module_factory(move || {
                    NativeIngress::new(Arc::new(PingoraIngressAdapter::new(
                        PingoraIngressConfig::new(bind_address),
                    )))
                }))
                .register_module(CaptureModule::new(Arc::clone(&capture)))
                .build();
        let composition =
            CompositionBuilder::new(CompositionId::new("test.pingora").expect("composition"))
                .register_block(block)
                .build()
                .expect("composition");
        let mut instance = composition
            .materialize(InstanceId::new("test.pingora").expect("instance id"))
            .expect("materialize");
        instance.start().expect("start block");
        let service = capture
            .service
            .lock()
            .expect("service capture")
            .clone()
            .expect("service contract");
        let ingress = capture
            .ingress
            .lock()
            .expect("ingress capture")
            .clone()
            .expect("ingress contract");
        let local_http_access = capture
            .local_http_access
            .lock()
            .expect("local ingress capture")
            .clone()
            .expect("local ingress access contract");
        Self {
            _tempdir: tempdir,
            instance,
            service,
            ingress,
            local_http_access,
            probe,
        }
    }
}

impl Drop for PingoraHarness {
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

pub(crate) fn ensure_service(harness: &PingoraHarness) -> fabric_resource_service::Service {
    harness
        .service
        .ensure_service(&scope(), &requirement())
        .expect("ensure service")
        .service
}

pub(crate) fn register_ready_target(
    harness: &PingoraHarness,
    service: &fabric_resource_service::Service,
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
                probe: Arc::clone(&harness.probe),
                status,
                body: body.as_bytes().to_vec(),
            })),
        )
        .expect("register service target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("mark target ready");
    target.id
}

pub(crate) fn register_ready_loopback_target(
    harness: &PingoraHarness,
    service: &fabric_resource_service::Service,
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
        .expect("register loopback service target");
    harness
        .service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("mark loopback target ready");
    target.id
}

pub(crate) fn ensure_route_and_access(
    harness: &PingoraHarness,
    service: &fabric_resource_service::Service,
) -> (
    fabric_resource_ingress::IngressRoute,
    fabric_resource_ingress::LocalHttpIngressAccess,
) {
    let route = harness
        .ingress
        .ensure_route(&IngressRouteTarget::for_service(service))
        .expect("ensure route");
    let access = harness
        .local_http_access
        .materialize_http_access(&route.id)
        .expect("materialize local access");
    (route, access)
}

fn service_db_path(root: &Path) -> PathBuf {
    root.join("services.sqlite")
}

fn requirement() -> ServiceRequirement {
    ServiceRequirement::new(ServiceProtocol::Http, "web").expect("service requirement")
}

fn scope() -> ServiceScope {
    ServiceScope::new("apps.notes.web").expect("scope")
}

pub(crate) fn spawn_loopback_server(status: u16, body: &'static str) -> LoopbackServer {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .expect("bind loopback server");
    let address = listener.local_addr().expect("loopback server address");
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
            .expect("write loopback response headers");
        stream
            .write_all(&body)
            .expect("write loopback response body");
        stream.flush().expect("flush loopback response");
    });
    LoopbackServer {
        thread: Some(thread),
        address,
    }
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
