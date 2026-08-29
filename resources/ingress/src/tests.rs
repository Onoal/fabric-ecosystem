use std::fs;
use std::sync::Arc;

use fabric_core::ContractId;
use fabric_resource_service::{ServiceId, ServiceProtocol, ServiceRequirement, ServiceScope};

use crate::{
    IngressContract, IngressError, IngressRoute, IngressRouteId, IngressRouteTarget,
    LocalHttpIngressAccess, LocalHttpIngressAccessContract, LocalHttpIngressAccessService,
    local_http_ingress_access_contract_id,
};

struct StubIngress;

impl crate::IngressService for StubIngress {
    fn ensure_route(&self, target: &IngressRouteTarget) -> Result<IngressRoute, IngressError> {
        Ok(IngressRoute::for_target(target))
    }

    fn get_route(&self, route_id: &IngressRouteId) -> Result<IngressRoute, IngressError> {
        Ok(IngressRoute {
            id: route_id.clone(),
            target: target(),
        })
    }

    fn resolve_route(&self, _service_id: &ServiceId) -> Result<IngressRoute, IngressError> {
        Ok(IngressRoute::for_target(&target()))
    }

    fn withdraw_route(&self, _route_id: &IngressRouteId) -> Result<(), IngressError> {
        Ok(())
    }
}

struct StubLocalAccess;

impl LocalHttpIngressAccessService for StubLocalAccess {
    fn materialize_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError> {
        Ok(LocalHttpIngressAccess {
            route_id: route_id.clone(),
            url: format!("http://127.0.0.1/{}", route_id.as_str()),
        })
    }

    fn resolve_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError> {
        self.materialize_http_access(route_id)
    }

    fn withdraw_http_access(&self, _route_id: &IngressRouteId) -> Result<(), IngressError> {
        Ok(())
    }
}

#[test]
fn ingress_route_id_is_semantic_and_stable_for_one_endpoint() {
    let target = target();
    let first = IngressRoute::for_target(&target);
    let second = IngressRoute::for_target(&target);

    assert_eq!(first.id, second.id);
    assert_eq!(first.target, target);
}

#[test]
fn ingress_and_local_access_contracts_remain_typed_semantic_seams() {
    let ingress = IngressContract::new(Arc::new(StubIngress));
    let local_access = LocalHttpIngressAccessContract::new(Arc::new(StubLocalAccess));
    let route = ingress.ensure_route(&target()).expect("ensure route");
    let access = local_access
        .materialize_http_access(&route.id)
        .expect("materialize local access");

    assert_eq!(access.route_id, route.id);
    assert_eq!(
        local_http_ingress_access_contract_id(),
        ContractId::new("fabric.resource.ingress.local-http-access").expect("contract id")
    );
}

#[test]
fn ingress_semantic_model_and_contract_do_not_leak_provider_or_upper_layer_terms() {
    let model = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/model.rs"))
        .expect("read model");
    let contract = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/contract.rs"))
        .expect("read contract");
    let local_access =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/local_access.rs"))
            .expect("read local access");

    for forbidden in [
        "SocketAddr",
        "TcpListener",
        "Pingora",
        "axum",
        "ServiceEndpointId",
        "Namespace",
        "Publication",
        "Gateway",
        "WorkerInstanceId",
        "AppId",
        "InstalledAppId",
    ] {
        assert!(
            !model.contains(forbidden)
                && !contract.contains(forbidden)
                && !local_access.contains(forbidden),
            "ingress semantic seam must not contain {forbidden}"
        );
    }
}

fn target() -> IngressRouteTarget {
    let scope = ServiceScope::new("apps.notes.web").expect("scope");
    let requirement = ServiceRequirement::new(ServiceProtocol::Http, "web").expect("requirement");
    let service_id = ServiceId::for_scope(&scope, &requirement);
    IngressRouteTarget::new(service_id, ServiceProtocol::Http)
}
