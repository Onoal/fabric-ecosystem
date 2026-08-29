use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource_ingress::{
    IngressContract, IngressRouteId, LocalHttpIngressAccessContract, ingress_contract_id,
    local_http_ingress_access_contract_id,
};
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::contract::{
    ConnectivityContract, ConnectivityService, connectivity_contract_key, connectivity_resource_id,
};
use crate::local_access::{
    LocalConnectivityAccess, LocalConnectivityAccessContract, LocalConnectivityAccessService,
    local_connectivity_access_contract_key,
};
use crate::native::persistence::ConnectivityDatabase;
use crate::{
    ConnectivityError, ConnectivityScope, PreparedReachability, Reachability, ReachabilityId,
};

#[derive(Clone)]
pub struct NativeConnectivityConfig {
    pub database_path: PathBuf,
}

pub struct NativeConnectivity {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    ingress_requirement: ContractRequirement<IngressContract>,
    local_http_access_requirement: ContractRequirement<LocalHttpIngressAccessContract>,
    config: NativeConnectivityConfig,
    shared: Arc<SharedConnectivityState>,
}

struct ConnectivityRuntimeState {
    health: Health,
    started: bool,
    runtime: Option<ConnectivityDatabase>,
    ingress: Option<IngressContract>,
    local_http_access: Option<LocalHttpIngressAccessContract>,
    active_reachabilities: BTreeMap<ReachabilityId, ActiveReachabilityState>,
}

#[derive(Clone)]
struct ActiveReachabilityState {
    reachability: Reachability,
    local_access: LocalConnectivityAccess,
}

struct SharedConnectivityState {
    inner: Mutex<ConnectivityRuntimeState>,
}

impl NativeConnectivity {
    pub fn new(config: NativeConnectivityConfig) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.connectivity.native")
                .expect("static connectivity module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            ingress_requirement: ContractRequirement::provisional(ingress_contract_id()),
            local_http_access_requirement: ContractRequirement::provisional(
                local_http_ingress_access_contract_id(),
            ),
            config,
            shared: Arc::new(SharedConnectivityState {
                inner: Mutex::new(ConnectivityRuntimeState {
                    health: Health::Unavailable,
                    started: false,
                    runtime: None,
                    ingress: None,
                    local_http_access: None,
                    active_reachabilities: BTreeMap::new(),
                }),
            }),
        }
    }
}

impl ModuleRuntime for NativeConnectivity {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![
            connectivity_contract_key().declaration(),
            local_connectivity_access_contract_key().declaration(),
        ]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.ingress_requirement.declaration().clone(),
            self.local_http_access_requirement.declaration().clone(),
        ]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let connectivity: Arc<dyn ConnectivityService> =
            Arc::clone(&self.shared) as Arc<dyn ConnectivityService>;
        let local_access: Arc<dyn LocalConnectivityAccessService> =
            Arc::clone(&self.shared) as Arc<dyn LocalConnectivityAccessService>;
        Ok(vec![
            ModuleContract::new(
                &connectivity_contract_key(),
                Arc::new(ConnectivityContract::new(connectivity)),
            ),
            ModuleContract::new(
                &local_connectivity_access_contract_key(),
                Arc::new(LocalConnectivityAccessContract::new(local_access)),
            ),
        ])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        let ingress = bindings
            .resolve(&self.ingress_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let local_http_access = bindings
            .resolve(&self.local_http_access_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let mut state = self.shared.inner.lock().expect("connectivity lock");
        state.ingress = Some((*ingress).clone());
        state.local_http_access = Some((*local_http_access).clone());
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        if let Some(parent) = self.config.database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ModuleError::new(format!("failed to create connectivity db parent: {error}"))
            })?;
        }
        let runtime = ConnectivityDatabase::open(&self.config.database_path)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let mut state = self.shared.inner.lock().expect("connectivity lock");
        state.runtime = Some(runtime);
        state.active_reachabilities.clear();
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        if let Some(shell) = &self.resource_registry {
            shell
                .register(self, ResourceDescriptor::new(connectivity_resource_id()))
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("connectivity lock");
        if state.runtime.is_none() || state.ingress.is_none() || state.local_http_access.is_none() {
            return Err(ModuleError::new(
                "connectivity dependencies are not initialized",
            ));
        }
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.resource_registry {
            let _ = shell.unregister(&self.module_id);
        }
        let mut state = self.shared.inner.lock().expect("connectivity lock");
        state.active_reachabilities.clear();
        state.runtime = None;
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("connectivity lock").health
    }
}

impl Module for NativeConnectivity {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new(self.config.clone()))
    }
}

impl ConnectivityService for SharedConnectivityState {
    fn ensure_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<PreparedReachability, ConnectivityError> {
        let mut state = self.inner.lock().expect("connectivity lock");
        let ingress = ensure_ingress(&state)?;
        let _ = ingress
            .get_route(ingress_route_id)
            .map_err(map_ingress_error)?;
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime.ensure_reachability(ingress_route_id, scope)
    }

    fn cleanup_prepared_reachability(
        &self,
        prepared: &PreparedReachability,
    ) -> Result<(), ConnectivityError> {
        let mut state = self.inner.lock().expect("connectivity lock");
        if state
            .active_reachabilities
            .contains_key(&prepared.reachability.id)
        {
            return Ok(());
        }
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime.cleanup_prepared_reachability(prepared)
    }

    fn get_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError> {
        let mut state = self.inner.lock().expect("connectivity lock");
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime.get_reachability(reachability_id)
    }

    fn resolve_reachability(
        &self,
        ingress_route_id: &IngressRouteId,
        scope: ConnectivityScope,
    ) -> Result<Reachability, ConnectivityError> {
        let mut state = self.inner.lock().expect("connectivity lock");
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime
            .find_reachability(ingress_route_id, scope)?
            .ok_or(ConnectivityError::NotFound)
    }

    fn activate_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<Reachability, ConnectivityError> {
        let mut state = self.inner.lock().expect("connectivity lock");
        ensure_started(&state)?;
        if let Some(active) = state.active_reachabilities.get(reachability_id) {
            return Ok(active.reachability.clone());
        }
        let reachability = ensure_runtime_mut(&mut state)?.get_reachability(reachability_id)?;
        let ingress = ensure_ingress(&state)?;
        let _ = ingress
            .get_route(&reachability.ingress_route_id)
            .map_err(map_ingress_error)?;
        let local_http_access = ensure_local_http_access(&state)?;
        let access = local_http_access
            .materialize_http_access(&reachability.ingress_route_id)
            .map_err(map_ingress_error)?;
        state.active_reachabilities.insert(
            reachability.id.clone(),
            ActiveReachabilityState {
                local_access: LocalConnectivityAccess {
                    reachability_id: reachability.id.clone(),
                    url: access.url,
                },
                reachability: reachability.clone(),
            },
        );
        Ok(reachability)
    }

    fn deactivate_reachability(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<(), ConnectivityError> {
        let mut state = self.inner.lock().expect("connectivity lock");
        ensure_started(&state)?;
        let Some(active) = state.active_reachabilities.remove(reachability_id) else {
            return Ok(());
        };
        let local_http_access = ensure_local_http_access(&state)?;
        match local_http_access.withdraw_http_access(&active.reachability.ingress_route_id) {
            Ok(()) | Err(fabric_resource_ingress::IngressError::NotFound) => Ok(()),
            Err(error) => {
                state
                    .active_reachabilities
                    .insert(reachability_id.clone(), active);
                Err(map_ingress_error(error))
            }
        }
    }
}

impl LocalConnectivityAccessService for SharedConnectivityState {
    fn resolve_local_access(
        &self,
        reachability_id: &ReachabilityId,
    ) -> Result<LocalConnectivityAccess, ConnectivityError> {
        let state = self.inner.lock().expect("connectivity lock");
        ensure_started(&state)?;
        state
            .active_reachabilities
            .get(reachability_id)
            .map(|active| active.local_access.clone())
            .ok_or(ConnectivityError::Inactive)
    }
}

fn ensure_started(state: &ConnectivityRuntimeState) -> Result<(), ConnectivityError> {
    if state.started {
        Ok(())
    } else {
        Err(ConnectivityError::Unavailable)
    }
}

fn ensure_runtime_mut(
    state: &mut ConnectivityRuntimeState,
) -> Result<&mut ConnectivityDatabase, ConnectivityError> {
    state.runtime.as_mut().ok_or(ConnectivityError::Unavailable)
}

fn ensure_ingress(state: &ConnectivityRuntimeState) -> Result<IngressContract, ConnectivityError> {
    state.ingress.clone().ok_or(ConnectivityError::Unavailable)
}

fn ensure_local_http_access(
    state: &ConnectivityRuntimeState,
) -> Result<LocalHttpIngressAccessContract, ConnectivityError> {
    state
        .local_http_access
        .clone()
        .ok_or(ConnectivityError::Unavailable)
}

fn map_ingress_error(error: fabric_resource_ingress::IngressError) -> ConnectivityError {
    match error {
        fabric_resource_ingress::IngressError::Unavailable => ConnectivityError::Unavailable,
        fabric_resource_ingress::IngressError::NotFound => ConnectivityError::NotFound,
        fabric_resource_ingress::IngressError::InvalidInput { message }
        | fabric_resource_ingress::IngressError::ListenerFailed { message }
        | fabric_resource_ingress::IngressError::DispatchFailed { message } => {
            ConnectivityError::activation_failed(message)
        }
    }
}
