use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use fabric_adapter_ingress_pingora::{PingoraIngressAdapter, PingoraIngressConfig};
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

use crate::{
    ConnectivityContract, ConnectivityError, ConnectivityScope, LocalConnectivityAccessContract,
    NativeConnectivity, NativeConnectivityConfig, Reachability,
};

#[test]
fn local_connectivity_scope_is_environment_neutral_and_persistable() {
    assert_eq!(ConnectivityScope::Local.as_str(), "local");
    assert_eq!(
        ConnectivityScope::parse("local").expect("local scope"),
        ConnectivityScope::Local
    );
    assert!(ConnectivityScope::parse("lan").is_err());
}

struct FakeRuntime {
    body: &'static str,
}

impl ServiceHttpTargetRuntimeService for FakeRuntime {
    fn dispatch_http(
        &self,
        _endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        request.validate()?;
        Ok(ServiceHttpResponse {
            status: 200,
            headers: Vec::new(),
            body: self.body.as_bytes().to_vec(),
        })
    }
}

#[derive(Default)]
struct Capture {
    service: Mutex<Option<ServiceContract>>,
    ingress: Mutex<Option<IngressContract>>,
    _local_http_access: Mutex<Option<LocalHttpIngressAccessContract>>,
    connectivity: Mutex<Option<ConnectivityContract>>,
    local_connectivity_access: Mutex<Option<LocalConnectivityAccessContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    service_requirement: ContractRequirement<ServiceContract>,
    ingress_requirement: ContractRequirement<IngressContract>,
    local_http_access_requirement: ContractRequirement<LocalHttpIngressAccessContract>,
    connectivity_requirement: ContractRequirement<ConnectivityContract>,
    local_connectivity_access_requirement: ContractRequirement<LocalConnectivityAccessContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("test.connectivity.capture").expect("module"),
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
                crate::connectivity_contract_id(),
            ),
            local_connectivity_access_requirement: ContractRequirement::provisional(
                crate::local_connectivity_access_contract_id(),
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
            self.connectivity_requirement.id().clone(),
            self.local_connectivity_access_requirement.id().clone(),
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
        let connectivity = bindings
            .resolve(&self.connectivity_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let local_connectivity_access = bindings
            .resolve(&self.local_connectivity_access_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.service.lock().expect("service") = Some((*service).clone());
        *self.capture.ingress.lock().expect("ingress") = Some((*ingress).clone());
        *self
            .capture
            ._local_http_access
            .lock()
            .expect("local ingress access") = Some((*local_http_access).clone());
        *self.capture.connectivity.lock().expect("connectivity") = Some((*connectivity).clone());
        *self
            .capture
            .local_connectivity_access
            .lock()
            .expect("local connectivity access") = Some((*local_connectivity_access).clone());
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

struct Harness {
    _tempdir: Option<TempDir>,
    instance: Instance,
    service: ServiceContract,
    ingress: IngressContract,
    connectivity: ConnectivityContract,
    local_connectivity_access: LocalConnectivityAccessContract,
}

impl Harness {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        let root = tempdir.path().to_path_buf();
        Self::from_root(&root, Some(tempdir))
    }

    fn from_existing_root(root: &std::path::Path) -> Self {
        Self::from_root(root, None)
    }

    fn from_root(root: &std::path::Path, tempdir: Option<TempDir>) -> Self {
        let capture = Arc::new(Capture::default());
        let block =
            BlockBuilder::new(BlockId::new("test.connectivity.block".to_owned()).expect("block"))
                .register_module(NativeServices::new(NativeServicesConfig {
                    database_path: root.join("services.sqlite"),
                }))
                .register_module(module_factory(|| {
                    NativeIngress::new(Arc::new(PingoraIngressAdapter::new(
                        PingoraIngressConfig::new(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))),
                    )))
                }))
                .register_module(NativeConnectivity::new(NativeConnectivityConfig {
                    database_path: root.join("connectivity.sqlite"),
                }))
                .register_module(CaptureModule::new(Arc::clone(&capture)))
                .build();
        let composition =
            CompositionBuilder::new(CompositionId::new("test.connectivity").expect("composition"))
                .register_block(block)
                .build()
                .expect("composition");
        let mut instance = composition
            .materialize(InstanceId::new("test.connectivity").expect("instance id"))
            .expect("materialize");
        instance.start().expect("start");
        let service = capture
            .service
            .lock()
            .expect("service")
            .clone()
            .expect("service contract");
        let ingress = capture
            .ingress
            .lock()
            .expect("ingress")
            .clone()
            .expect("ingress contract");
        let connectivity = capture
            .connectivity
            .lock()
            .expect("connectivity")
            .clone()
            .expect("connectivity contract");
        let local_connectivity_access = capture
            .local_connectivity_access
            .lock()
            .expect("local connectivity access")
            .clone()
            .expect("local connectivity access contract");
        Self {
            _tempdir: tempdir,
            instance,
            service,
            ingress,
            connectivity,
            local_connectivity_access,
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.instance.stop();
    }
}

fn scope() -> ServiceScope {
    ServiceScope::new("home.test.app.alpha").expect("scope")
}

fn requirement(name: &str) -> ServiceRequirement {
    ServiceRequirement::new(ServiceProtocol::Http, name).expect("service requirement")
}

fn ensure_service(harness: &Harness, name: &str) -> fabric_resource_service::Service {
    harness
        .service
        .ensure_service(&scope(), &requirement(name))
        .expect("ensure service")
        .service
}

fn register_ready_target(
    service_contract: &ServiceContract,
    service: &fabric_resource_service::Service,
    suffix: &str,
    body: &'static str,
) {
    let target = ServiceTarget {
        id: ServiceTargetId::new(format!("connectivity_target_{suffix}")).expect("target id"),
        endpoint_id: service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service_contract
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service.id.clone(),
                target: target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FakeRuntime { body })),
        )
        .expect("register target");
    service_contract
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service.id.clone(),
            target_id: target.id,
        })
        .expect("ready target");
}

fn ensure_route(
    harness: &Harness,
    service: &fabric_resource_service::Service,
) -> fabric_resource_ingress::IngressRoute {
    harness
        .ingress
        .ensure_route(&IngressRouteTarget::for_service(service))
        .expect("ensure ingress route")
}

fn ensure_local_reachability(
    harness: &Harness,
    route: &fabric_resource_ingress::IngressRoute,
) -> Reachability {
    harness
        .connectivity
        .ensure_reachability(&route.id, ConnectivityScope::Local)
        .expect("ensure reachability")
        .reachability
}

#[test]
fn reachability_identity_is_stable_and_survives_reopen() {
    let tempdir = TempDir::new().expect("tempdir");
    let root = tempdir.path().to_path_buf();

    let first_reachability_id = {
        let harness = Harness::from_existing_root(&root);
        let service = ensure_service(&harness, "web");
        register_ready_target(&harness.service, &service, "first", "first");
        let route = ensure_route(&harness, &service);
        ensure_local_reachability(&harness, &route).id
    };

    let harness = Harness::from_existing_root(&root);
    let service = ensure_service(&harness, "web");
    register_ready_target(&harness.service, &service, "second", "second");
    let route = ensure_route(&harness, &service);
    let second_reachability = ensure_local_reachability(&harness, &route);

    assert_eq!(second_reachability.id, first_reachability_id);
}

#[test]
fn connectivity_activates_resolves_and_deactivates_without_apps() {
    let harness = Harness::new();
    let service = ensure_service(&harness, "web");
    register_ready_target(
        &harness.service,
        &service,
        "connectivity_1",
        "connectivity-ok",
    );
    let route = ensure_route(&harness, &service);
    let reachability = ensure_local_reachability(&harness, &route);

    assert_eq!(
        harness
            .local_connectivity_access
            .resolve_local_access(&reachability.id),
        Err(ConnectivityError::Inactive)
    );

    let active = harness
        .connectivity
        .activate_reachability(&reachability.id)
        .expect("activate reachability");
    assert_eq!(active, reachability);

    let access = harness
        .local_connectivity_access
        .resolve_local_access(&reachability.id)
        .expect("resolve local access");
    let response = ureq::get(&access.url).call().expect("ingress request");
    assert_eq!(
        response.into_body().read_to_string().expect("body"),
        "connectivity-ok"
    );

    harness
        .connectivity
        .deactivate_reachability(&reachability.id)
        .expect("deactivate reachability");
    assert_eq!(
        harness
            .local_connectivity_access
            .resolve_local_access(&reachability.id),
        Err(ConnectivityError::Inactive)
    );
    assert_eq!(
        harness
            .connectivity
            .get_reachability(&reachability.id)
            .expect("durable reachability"),
        reachability
    );
}

#[test]
fn restart_preserves_reachability_but_not_stale_local_access() {
    let tempdir = TempDir::new().expect("tempdir");
    let root = tempdir.path().to_path_buf();

    let (reachability_id, first_url) = {
        let harness = Harness::from_existing_root(&root);
        let service = ensure_service(&harness, "web");
        register_ready_target(&harness.service, &service, "restart_one", "restart-one");
        let route = ensure_route(&harness, &service);
        let reachability = ensure_local_reachability(&harness, &route);
        harness
            .connectivity
            .activate_reachability(&reachability.id)
            .expect("activate");
        let access = harness
            .local_connectivity_access
            .resolve_local_access(&reachability.id)
            .expect("local access");
        (reachability.id, access.url)
    };

    let harness = Harness::from_existing_root(&root);
    let service = ensure_service(&harness, "web");
    register_ready_target(&harness.service, &service, "restart_two", "restart-two");
    let route = ensure_route(&harness, &service);
    let reachability = ensure_local_reachability(&harness, &route);

    assert_eq!(reachability.id, reachability_id);
    assert_eq!(
        harness
            .local_connectivity_access
            .resolve_local_access(&reachability.id),
        Err(ConnectivityError::Inactive)
    );

    harness
        .connectivity
        .activate_reachability(&reachability.id)
        .expect("reactivate");
    let access = harness
        .local_connectivity_access
        .resolve_local_access(&reachability.id)
        .expect("fresh local access");
    assert_ne!(access.url, first_url);

    let response = ureq::get(&access.url)
        .call()
        .expect("request after restart");
    assert_eq!(
        response.into_body().read_to_string().expect("body"),
        "restart-two"
    );
}

#[test]
fn different_ingress_routes_have_distinct_reachability_ids() {
    let harness = Harness::new();
    let web_service = ensure_service(&harness, "web");
    register_ready_target(&harness.service, &web_service, "web", "web");
    let web_route = ensure_route(&harness, &web_service);
    let web_reachability = ensure_local_reachability(&harness, &web_route);

    let api_service = ensure_service(&harness, "api");
    register_ready_target(&harness.service, &api_service, "api", "api");
    let api_route = ensure_route(&harness, &api_service);
    let api_reachability = ensure_local_reachability(&harness, &api_route);

    assert_ne!(web_reachability.id, api_reachability.id);
}

#[test]
fn connectivity_semantic_model_and_contract_do_not_leak_names_or_transport_material() {
    let model = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/model.rs"))
        .expect("read model");
    let contract = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/contract.rs"))
        .expect("read contract");
    let local_access =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/local_access.rs"))
            .expect("read local access");

    for forbidden in [
        "Namespace",
        "ServiceId",
        "url: String",
        "SocketAddr",
        "LoopbackHttp",
        "AppId",
        "InstalledAppId",
        "WorkerInstanceId",
        "Publication",
        "Gateway",
    ] {
        assert!(
            !model.contains(forbidden) && !contract.contains(forbidden),
            "connectivity semantic model/contract must not contain {forbidden}"
        );
    }

    assert!(
        local_access.contains("url: String"),
        "local connectivity access seam must remain the local URL holder"
    );
}
