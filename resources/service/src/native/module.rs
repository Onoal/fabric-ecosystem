use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource::ResourceId;
use fabric_resource_registry::{
    ResourceConfigurationFacet, ResourceConfigurationKind, ResourceDescriptor, ResourceInspection,
    ResourceInspectionEntry, ResourceInspectionFacet, ResourceRegistry,
    resource_registry_contract_id,
};

use crate::contract::{ServiceContract, ServiceService, service_contract_key};
use crate::native::persistence::ServiceDatabase;
use crate::{
    MarkServiceTargetDrainingRequest, MarkServiceTargetReadyRequest, PreparedService,
    RegisterServiceTargetRequest, Service, ServiceError, ServiceHttpRequest, ServiceHttpResponse,
    ServiceHttpTargetRuntime, ServiceId, ServiceLiveEndpoint, ServiceRequirement, ServiceScope,
    ServiceTarget, ServiceTargetId, ServiceTargetState, WithdrawServiceTargetRequest,
};

#[derive(Clone)]
pub struct NativeServicesConfig {
    pub database_path: PathBuf,
}

pub struct NativeServices {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    config: NativeServicesConfig,
    shared: Arc<SharedServicesState>,
}

pub(crate) struct SharedServicesState {
    inner: Mutex<ServiceState>,
}

struct ServiceState {
    health: Health,
    started: bool,
    effective_database_path: Option<PathBuf>,
    runtime: Option<ServiceDatabase>,
    targets: BTreeMap<ServiceId, BTreeMap<String, RegisteredRuntimeTarget>>,
}

#[derive(Clone)]
struct RegisteredRuntimeTarget {
    target: ServiceTarget,
    runtime: ServiceHttpTargetRuntime,
}

impl NativeServices {
    pub fn new(config: NativeServicesConfig) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.service.native")
                .expect("static service module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            config,
            shared: Arc::new(SharedServicesState {
                inner: Mutex::new(ServiceState {
                    health: Health::Unavailable,
                    started: false,
                    effective_database_path: None,
                    runtime: None,
                    targets: BTreeMap::new(),
                }),
            }),
        }
    }
}

impl ModuleRuntime for NativeServices {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![service_contract_key().declaration()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let service: Arc<dyn ServiceService> = Arc::clone(&self.shared) as Arc<dyn ServiceService>;
        Ok(vec![ModuleContract::new(
            &service_contract_key(),
            Arc::new(ServiceContract::new(service)),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        if let Some(parent) = self.config.database_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| ModuleError::new(format!("create service db parent: {error}")))?;
        }
        let runtime = ServiceDatabase::open(&self.config.database_path)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let mut state = self.shared.inner.lock().expect("service lock");
        state.runtime = Some(runtime);
        state.targets.clear();
        state.started = false;
        state.health = Health::Unavailable;
        state.effective_database_path = Some(self.config.database_path.clone());
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        if let Some(shell) = &self.resource_registry {
            let shared = Arc::clone(&self.shared);
            shell
                .register(
                    self,
                    ResourceDescriptor::new(
                        ResourceId::new("service").expect("static resource id"),
                    )
                    .with_configuration(ResourceConfigurationFacet::typed_with::<
                        NativeServicesConfig,
                    >(
                        ResourceConfigurationKind::new("service.native")
                            .expect("static resource configuration kind"),
                        move |config| shared.consume_configuration(config),
                    ))
                    .with_inspection(ResourceInspectionFacet::new({
                        let shared = Arc::clone(&self.shared);
                        move || shared.inspect()
                    })),
                )
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("service lock");
        if state.runtime.is_none() {
            return Err(ModuleError::new("service runtime is not initialized"));
        }
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.resource_registry {
            let _ = shell.unregister(&self.module_id);
        }
        let mut state = self.shared.inner.lock().expect("service lock");
        state.targets.clear();
        state.runtime = None;
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("service lock").health
    }
}

impl Module for NativeServices {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new(self.config.clone()))
    }
}

impl ServiceService for SharedServicesState {
    fn ensure_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<PreparedService, ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime.ensure_service(scope, requirement)
    }

    fn cleanup_prepared_service(&self, prepared: &PreparedService) -> Result<(), ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        if state
            .targets
            .get(&prepared.service.id)
            .is_some_and(|targets| !targets.is_empty())
        {
            return Ok(());
        }
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime.cleanup_prepared_service(prepared)
    }

    fn resolve_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<Service, ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime
            .find_service(scope, requirement)?
            .ok_or(ServiceError::NotFound)
    }

    fn get_service(&self, service_id: &ServiceId) -> Result<Service, ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        let runtime = ensure_runtime_mut(&mut state)?;
        runtime.get_service(service_id)
    }

    fn resolve_live_endpoint(
        &self,
        service_id: &ServiceId,
    ) -> Result<ServiceLiveEndpoint, ServiceError> {
        let state = self.inner.lock().expect("service lock");
        ensure_started(&state)?;
        let service = state
            .runtime
            .as_ref()
            .ok_or(ServiceError::Unavailable)?
            .get_service(service_id)?;
        let registered = resolve_ready_target(&state, service_id)?;
        Ok(ServiceLiveEndpoint {
            service_id: service.id,
            target_id: registered.target.id,
            protocol: service.requirement.protocol,
        })
    }

    fn list_targets(&self, service_id: &ServiceId) -> Result<Vec<ServiceTarget>, ServiceError> {
        let state = self.inner.lock().expect("service lock");
        ensure_started(&state)?;
        Ok(state
            .targets
            .get(service_id)
            .map(|targets| {
                targets
                    .values()
                    .map(|target| target.target.clone())
                    .collect()
            })
            .unwrap_or_default())
    }

    fn register_target(
        &self,
        request: RegisterServiceTargetRequest,
        runtime: ServiceHttpTargetRuntime,
    ) -> Result<(), ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        let service = ensure_runtime_mut(&mut state)?.get_service(&request.service_id)?;
        validate_target_registration(&service, &request.target)?;
        state.targets.entry(request.service_id).or_default().insert(
            request.target.id.as_str().to_owned(),
            RegisteredRuntimeTarget {
                target: request.target,
                runtime,
            },
        );
        Ok(())
    }

    fn mark_target_ready(
        &self,
        request: MarkServiceTargetReadyRequest,
    ) -> Result<(), ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        ensure_started(&state)?;
        let target = lookup_target_mut(&mut state, &request.service_id, &request.target_id)?;
        target.target.state = ServiceTargetState::Ready;
        Ok(())
    }

    fn mark_target_draining(
        &self,
        request: MarkServiceTargetDrainingRequest,
    ) -> Result<(), ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        ensure_started(&state)?;
        let target = lookup_target_mut(&mut state, &request.service_id, &request.target_id)?;
        target.target.state = ServiceTargetState::Draining;
        Ok(())
    }

    fn withdraw_target(&self, request: WithdrawServiceTargetRequest) -> Result<(), ServiceError> {
        let mut state = self.inner.lock().expect("service lock");
        ensure_started(&state)?;
        if let Some(targets) = state.targets.get_mut(&request.service_id) {
            targets.remove(request.target_id.as_str());
            if targets.is_empty() {
                state.targets.remove(&request.service_id);
            }
        }
        Ok(())
    }

    fn dispatch_http(
        &self,
        service_id: &ServiceId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        request.validate()?;
        let registered = {
            let state = self.inner.lock().expect("service lock");
            ensure_started(&state)?;
            resolve_ready_target(&state, service_id)?
        };
        registered
            .runtime
            .dispatch_http(&registered.target.endpoint_id, request)
    }
}

impl SharedServicesState {
    fn consume_configuration(
        &self,
        config: &NativeServicesConfig,
    ) -> Result<(), fabric_resource_registry::ResourceRegistryError> {
        let mut state = self.inner.lock().expect("service lock");
        if state.runtime.is_none() {
            return Err(
                fabric_resource_registry::ResourceRegistryError::ConfigurationRejected {
                    message: "service runtime is not initialized".to_owned(),
                },
            );
        }
        if let Some(existing) = state.effective_database_path.as_ref() {
            if existing != &config.database_path {
                return Err(
                    fabric_resource_registry::ResourceRegistryError::ConfigurationRejected {
                        message:
                            "service configuration change is not supported after initialization"
                                .to_owned(),
                    },
                );
            }
        } else {
            state.effective_database_path = Some(config.database_path.clone());
        }
        Ok(())
    }

    fn inspect(
        &self,
    ) -> Result<ResourceInspection, fabric_resource_registry::ResourceRegistryError> {
        let state = self.inner.lock().expect("service lock");
        ResourceInspection::new(vec![
            ResourceInspectionEntry::public(
                "configured",
                state.effective_database_path.is_some().to_string(),
            )?,
            ResourceInspectionEntry::public("resource", "service")?,
            ResourceInspectionEntry::public("started", state.started.to_string())?,
        ])
    }
}

fn ensure_started(state: &ServiceState) -> Result<(), ServiceError> {
    if state.started {
        Ok(())
    } else {
        Err(ServiceError::Unavailable)
    }
}

fn ensure_runtime_mut(state: &mut ServiceState) -> Result<&mut ServiceDatabase, ServiceError> {
    ensure_started(state)?;
    state.runtime.as_mut().ok_or(ServiceError::Unavailable)
}

fn validate_target_registration(
    service: &Service,
    target: &ServiceTarget,
) -> Result<(), ServiceError> {
    if target.endpoint_id != service.endpoint.id {
        return Err(ServiceError::invalid_input(
            "service target endpoint does not belong to the requested service",
        ));
    }
    if target.state != ServiceTargetState::Registered {
        return Err(ServiceError::invalid_input(
            "service target must start in the registered state",
        ));
    }
    Ok(())
}

fn lookup_target_mut<'a>(
    state: &'a mut ServiceState,
    service_id: &ServiceId,
    target_id: &ServiceTargetId,
) -> Result<&'a mut RegisteredRuntimeTarget, ServiceError> {
    state
        .targets
        .get_mut(service_id)
        .and_then(|targets| targets.get_mut(target_id.as_str()))
        .ok_or(ServiceError::NotFound)
}

fn resolve_ready_target(
    state: &ServiceState,
    service_id: &ServiceId,
) -> Result<RegisteredRuntimeTarget, ServiceError> {
    state
        .targets
        .get(service_id)
        .and_then(|targets| {
            targets
                .values()
                .find(|target| target.target.state == ServiceTargetState::Ready)
                .cloned()
        })
        .ok_or(ServiceError::NoTarget)
}
