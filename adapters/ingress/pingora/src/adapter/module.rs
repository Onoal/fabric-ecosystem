use std::sync::Arc;

use fabric_core::{Health, ModuleError};
use fabric_resource_ingress::{
    IngressAdapter, IngressError, IngressRoute, IngressRouteId, IngressRouteTarget, IngressService,
    LocalHttpIngressAccess, LocalHttpIngressAccessService,
};
use fabric_resource_service::{Service, ServiceContract, ServiceError, ServiceId};

use crate::PingoraIngressConfig;

use super::runtime::{shutdown_runtime, spawn_runtime, wait_for_readiness};
use super::shared::SharedPingoraState;

pub struct PingoraIngressAdapter {
    config: PingoraIngressConfig,
    shared: Arc<SharedPingoraState>,
}

impl PingoraIngressAdapter {
    pub fn new(config: PingoraIngressConfig) -> Self {
        Self {
            config,
            shared: Arc::new(SharedPingoraState::new()),
        }
    }
}

impl IngressAdapter for PingoraIngressAdapter {
    fn ingress_service(&self) -> Arc<dyn IngressService> {
        self.shared.clone()
    }

    fn local_http_ingress_access_service(&self) -> Arc<dyn LocalHttpIngressAccessService> {
        self.shared.clone()
    }

    fn bind_service(&self, service: ServiceContract) {
        self.shared
            .inner
            .lock()
            .expect("pingora ingress lock")
            .service = Some(service);
    }

    fn initialize(&self) -> Result<(), ModuleError> {
        if !self.config.bind_address.ip().is_loopback() {
            return Err(ModuleError::new("local ingress must bind only to loopback"));
        }
        let mut state = self.shared.inner.lock().expect("pingora ingress lock");
        state.routes.clear();
        state.routes_by_service.clear();
        state.local_http_access.clear();
        state.runtime = None;
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&self) -> Result<(), ModuleError> {
        let previous_port = {
            let state = self.shared.inner.lock().expect("pingora ingress lock");
            state
                .service
                .clone()
                .ok_or_else(|| ModuleError::new("ingress service dependency is not bound"))?;
            state.previous_port
        };

        let runtime = spawn_runtime(Arc::clone(&self.shared), &self.config, previous_port)
            .map_err(ModuleError::new)?;
        if let Err(error) = wait_for_readiness(&runtime, &self.config) {
            let _ = shutdown_runtime(runtime, self.config.shutdown_timeout);
            return Err(ModuleError::new(error));
        }

        let mut state = self.shared.inner.lock().expect("pingora ingress lock");
        state.previous_port = Some(runtime.listen_address.port());
        state.runtime = Some(runtime);
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&self) {
        let runtime = {
            let mut state = self.shared.inner.lock().expect("pingora ingress lock");
            state.routes.clear();
            state.routes_by_service.clear();
            state.local_http_access.clear();
            state.started = false;
            state.health = Health::Unavailable;
            state.runtime.take()
        };
        if let Some(runtime) = runtime {
            let _ = shutdown_runtime(runtime, self.config.shutdown_timeout);
        }
    }

    fn health(&self) -> Health {
        self.shared
            .inner
            .lock()
            .expect("pingora ingress lock")
            .health
    }
}

impl IngressService for SharedPingoraState {
    fn ensure_route(&self, target: &IngressRouteTarget) -> Result<IngressRoute, IngressError> {
        let mut state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        let service = state.service.clone().ok_or(IngressError::Unavailable)?;
        let service_record = service
            .get_service(&target.service_id)
            .map_err(map_service_error)?;
        ensure_route_matches_service(target, &service_record)?;
        if let Some(route_id) = state.routes_by_service.get(&target.service_id) {
            return state
                .routes
                .get(route_id)
                .cloned()
                .ok_or(IngressError::Unavailable);
        }
        let route = IngressRoute::for_target(target);
        state.routes.insert(route.id.clone(), route.clone());
        state
            .routes_by_service
            .insert(target.service_id.clone(), route.id.clone());
        Ok(route)
    }

    fn get_route(&self, route_id: &IngressRouteId) -> Result<IngressRoute, IngressError> {
        let state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        state
            .routes
            .get(route_id)
            .cloned()
            .ok_or(IngressError::NotFound)
    }

    fn resolve_route(&self, service_id: &ServiceId) -> Result<IngressRoute, IngressError> {
        let state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        let route_id = state
            .routes_by_service
            .get(service_id)
            .ok_or(IngressError::NotFound)?;
        state
            .routes
            .get(route_id)
            .cloned()
            .ok_or(IngressError::NotFound)
    }

    fn withdraw_route(&self, route_id: &IngressRouteId) -> Result<(), IngressError> {
        let mut state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        let Some(route) = state.routes.remove(route_id) else {
            return Err(IngressError::NotFound);
        };
        state.routes_by_service.remove(&route.target.service_id);
        state.local_http_access.remove(route_id);
        Ok(())
    }
}

impl LocalHttpIngressAccessService for SharedPingoraState {
    fn materialize_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError> {
        let mut state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        if let Some(access) = state.local_http_access.get(route_id) {
            return Ok(access.clone());
        }
        if !state.routes.contains_key(route_id) {
            return Err(IngressError::NotFound);
        }
        let listen_address = runtime_address(&state)?;
        let access = LocalHttpIngressAccess {
            route_id: route_id.clone(),
            url: format!("http://{listen_address}/{}", route_id.as_str()),
        };
        state
            .local_http_access
            .insert(route_id.clone(), access.clone());
        Ok(access)
    }

    fn resolve_http_access(
        &self,
        route_id: &IngressRouteId,
    ) -> Result<LocalHttpIngressAccess, IngressError> {
        let state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        state
            .local_http_access
            .get(route_id)
            .cloned()
            .ok_or(IngressError::NotFound)
    }

    fn withdraw_http_access(&self, route_id: &IngressRouteId) -> Result<(), IngressError> {
        let mut state = self.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        if state.local_http_access.remove(route_id).is_none() {
            return Err(IngressError::NotFound);
        }
        Ok(())
    }
}

fn ensure_started(state: &super::shared::PingoraState) -> Result<(), IngressError> {
    if state.started {
        Ok(())
    } else {
        Err(IngressError::Unavailable)
    }
}

fn runtime_address(
    state: &super::shared::PingoraState,
) -> Result<std::net::SocketAddr, IngressError> {
    state
        .runtime
        .as_ref()
        .map(|runtime| runtime.listen_address)
        .ok_or(IngressError::Unavailable)
}

fn ensure_route_matches_service(
    target: &IngressRouteTarget,
    service: &Service,
) -> Result<(), IngressError> {
    if service.id != target.service_id {
        return Err(IngressError::InvalidInput {
            message: "ingress route target does not match service identity".to_owned(),
        });
    }
    if service.requirement.protocol != target.protocol {
        return Err(IngressError::InvalidInput {
            message: "ingress route target protocol does not match service protocol".to_owned(),
        });
    }
    Ok(())
}

fn map_service_error(error: ServiceError) -> IngressError {
    match error {
        ServiceError::Unavailable => IngressError::Unavailable,
        ServiceError::NotFound => IngressError::NotFound,
        ServiceError::NoTarget => IngressError::Unavailable,
        ServiceError::InvalidInput { message }
        | ServiceError::Integrity { message }
        | ServiceError::Persistence { message }
        | ServiceError::PublishFailed { message }
        | ServiceError::DispatchFailed { message } => IngressError::DispatchFailed { message },
    }
}
