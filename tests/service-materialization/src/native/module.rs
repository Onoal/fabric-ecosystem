use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_component::{
    ComponentRuntime, Surface, SurfaceId, SurfaceRegistry, component_runtime_contract_id,
    surface_contract_id,
};
use fabric_component_gateway::{Gateway, GatewayEntry, gateway_contract_id};
use fabric_component_namespace::NamespaceName;
use fabric_component_publication::{Publication, PublicationContract, publication_contract_id};
use fabric_core::{
    ContractRequirement, Health, InstanceId, Module, ModuleBindings, ModuleContract, ModuleError,
    ModuleId, ModuleRuntime,
};
use fabric_resource_connectivity::{
    ConnectivityContract, ConnectivityScope, LocalConnectivityAccessContract,
    connectivity_contract_id, local_connectivity_access_contract_id,
};
use fabric_resource_ingress::{
    IngressContract, IngressError, IngressRouteTarget, ingress_contract_id,
};
use fabric_resource_service::{
    ServiceContract, ServiceError, ServiceId, ServiceProtocol, service_contract_id,
};

use crate::contract::{
    ServiceMaterializationContract, ServiceMaterializationService,
    service_materialization_contract_key,
};
use crate::error::ServiceMaterializationError;
use crate::gateway::{
    GatewayExposure, GatewayExposureAvailability, GatewayExposureContract, GatewayExposureService,
    gateway_exposure_contract_key,
};
use crate::local::{
    LocalGatewayExposure, LocalGatewayExposureContract, LocalGatewayExposureService,
    local_gateway_exposure_contract_key,
};
use crate::model::{MaterializedServicePublication, ServiceBackedSurface};
use crate::naming::{
    LocalNameProjection, LocalNameResolution, LocalNameResolutionContract,
    LocalNameResolutionService, LocalNetworkName, local_name_resolution_contract_key,
};

pub struct ServiceMaterializationModule {
    module_id: ModuleId,
    component_runtime_requirement: ContractRequirement<ComponentRuntime>,
    surface_requirement: ContractRequirement<SurfaceRegistry>,
    publication_requirement: ContractRequirement<PublicationContract>,
    gateway_requirement: ContractRequirement<Gateway>,
    service_requirement: ContractRequirement<ServiceContract>,
    ingress_requirement: ContractRequirement<IngressContract>,
    connectivity_requirement: ContractRequirement<ConnectivityContract>,
    local_connectivity_access_requirement: ContractRequirement<LocalConnectivityAccessContract>,
    shared: Arc<SharedMaterializationState>,
}

struct SharedMaterializationState {
    inner: Mutex<MaterializationState>,
}

struct MaterializationState {
    instance_id: Option<InstanceId>,
    surfaces: Option<SurfaceRegistry>,
    publications: Option<PublicationContract>,
    gateway: Option<Gateway>,
    service: Option<ServiceContract>,
    ingress: Option<IngressContract>,
    connectivity: Option<ConnectivityContract>,
    local_connectivity_access: Option<LocalConnectivityAccessContract>,
    available: bool,
    service_backings: BTreeMap<SurfaceId, ServiceBackedSurface>,
    materialized_publications: BTreeMap<NamespaceName, MaterializedServicePublication>,
    local_name_projections: BTreeMap<LocalNetworkName, LocalNameProjection>,
}

impl ServiceMaterializationModule {
    pub fn new() -> Self {
        Self {
            module_id: ModuleId::new("fabric.composition.service-materialization.module")
                .expect("static service materialization module id"),
            component_runtime_requirement: ContractRequirement::provisional(
                component_runtime_contract_id(),
            ),
            surface_requirement: ContractRequirement::provisional(surface_contract_id()),
            publication_requirement: ContractRequirement::provisional(publication_contract_id()),
            gateway_requirement: ContractRequirement::provisional(gateway_contract_id()),
            service_requirement: ContractRequirement::provisional(service_contract_id()),
            ingress_requirement: ContractRequirement::provisional(ingress_contract_id()),
            connectivity_requirement: ContractRequirement::provisional(connectivity_contract_id()),
            local_connectivity_access_requirement: ContractRequirement::provisional(
                local_connectivity_access_contract_id(),
            ),
            shared: Arc::new(SharedMaterializationState {
                inner: Mutex::new(MaterializationState {
                    instance_id: None,
                    surfaces: None,
                    publications: None,
                    gateway: None,
                    service: None,
                    ingress: None,
                    connectivity: None,
                    local_connectivity_access: None,
                    available: false,
                    service_backings: BTreeMap::new(),
                    materialized_publications: BTreeMap::new(),
                    local_name_projections: BTreeMap::new(),
                }),
            }),
        }
    }
}

impl Default for ServiceMaterializationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRuntime for ServiceMaterializationModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![
            service_materialization_contract_key().declaration(),
            gateway_exposure_contract_key().declaration(),
            local_gateway_exposure_contract_key().declaration(),
            local_name_resolution_contract_key().declaration(),
        ]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.component_runtime_requirement.declaration().clone(),
            self.surface_requirement.declaration().clone(),
            self.publication_requirement.declaration().clone(),
            self.gateway_requirement.declaration().clone(),
            self.service_requirement.declaration().clone(),
            self.ingress_requirement.declaration().clone(),
            self.connectivity_requirement.declaration().clone(),
            self.local_connectivity_access_requirement
                .declaration()
                .clone(),
        ]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let service: Arc<dyn ServiceMaterializationService> = self.shared.clone();
        let gateway_exposure: Arc<dyn GatewayExposureService> = self.shared.clone();
        let local_gateway_exposure: Arc<dyn LocalGatewayExposureService> = self.shared.clone();
        let local_name_resolution: Arc<dyn LocalNameResolutionService> = self.shared.clone();
        Ok(vec![
            ModuleContract::new(
                &service_materialization_contract_key(),
                Arc::new(ServiceMaterializationContract::new(service)),
            ),
            ModuleContract::new(
                &gateway_exposure_contract_key(),
                Arc::new(GatewayExposureContract::new(gateway_exposure)),
            ),
            ModuleContract::new(
                &local_gateway_exposure_contract_key(),
                Arc::new(LocalGatewayExposureContract::new(local_gateway_exposure)),
            ),
            ModuleContract::new(
                &local_name_resolution_contract_key(),
                Arc::new(LocalNameResolutionContract::new(local_name_resolution)),
            ),
        ])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let component_runtime = bindings
            .resolve(&self.component_runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let surfaces = bindings
            .resolve(&self.surface_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let publications = bindings
            .resolve(&self.publication_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let gateway = bindings
            .resolve(&self.gateway_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let service = bindings
            .resolve(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let ingress = bindings
            .resolve(&self.ingress_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let connectivity = bindings
            .resolve(&self.connectivity_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let local_connectivity_access = bindings
            .resolve(&self.local_connectivity_access_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;

        let mut state = self
            .shared
            .inner
            .lock()
            .expect("materialization state lock");
        state.instance_id = Some(component_runtime.instance_id());
        state.surfaces = Some((*surfaces).clone());
        state.publications = Some((*publications).clone());
        state.gateway = Some((*gateway).clone());
        state.service = Some((*service).clone());
        state.ingress = Some((*ingress).clone());
        state.connectivity = Some((*connectivity).clone());
        state.local_connectivity_access = Some((*local_connectivity_access).clone());
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.shared
            .inner
            .lock()
            .expect("materialization state lock")
            .available = true;
        Ok(())
    }

    fn stop(&mut self) {
        let mut state = self
            .shared
            .inner
            .lock()
            .expect("materialization state lock");
        state.available = false;
        state.service_backings.clear();
        state.materialized_publications.clear();
        state.local_name_projections.clear();
    }

    fn health(&self) -> Health {
        if self
            .shared
            .inner
            .lock()
            .expect("materialization state lock")
            .available
        {
            Health::Healthy
        } else {
            Health::Unavailable
        }
    }
}

impl Module for ServiceMaterializationModule {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new())
    }
}

impl ServiceMaterializationService for SharedMaterializationState {
    fn bind_service(
        &self,
        surface: Surface,
        service_id: ServiceId,
        protocol: ServiceProtocol,
    ) -> Result<ServiceBackedSurface, ServiceMaterializationError> {
        let (instance_id, surfaces, existing) = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            (
                state.instance_id(),
                state.surfaces.clone().expect("surfaces bound"),
                state.service_backings.get(surface.surface_id()).cloned(),
            )
        };

        ensure_surface_instance(&instance_id, &surface)?;
        let registered_surface = surfaces.surface(surface.surface_id())?;
        if registered_surface != surface {
            return Err(ServiceMaterializationError::from(
                fabric_component::ComponentError::UnknownSurface(surface.surface_id().clone()),
            ));
        }

        if let Some(existing) = existing {
            return reconcile_existing_backing(existing, &service_id, protocol);
        }

        let backing = ServiceBackedSurface::new(surface, service_id, protocol);
        let mut state = self.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        match state
            .service_backings
            .get(backing.surface().surface_id())
            .cloned()
        {
            Some(existing) => reconcile_existing_backing(existing, backing.service_id(), protocol),
            None => {
                state
                    .service_backings
                    .insert(backing.surface().surface_id().clone(), backing.clone());
                Ok(backing)
            }
        }
    }

    fn service_backing(
        &self,
        surface_id: &SurfaceId,
    ) -> Result<ServiceBackedSurface, ServiceMaterializationError> {
        let state = self.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        state
            .service_backings
            .get(surface_id)
            .cloned()
            .ok_or_else(|| ServiceMaterializationError::UnknownServiceBacking(surface_id.clone()))
    }

    fn materialize_publication(
        &self,
        publication: Publication,
    ) -> Result<MaterializedServicePublication, ServiceMaterializationError> {
        let name = publication.claim().name().clone();
        let (publications, ingress, backing, cached) = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            (
                state.publications.clone().expect("publications bound"),
                state.ingress.clone().expect("ingress bound"),
                state
                    .service_backings
                    .get(publication.surface().surface_id())
                    .cloned(),
                state.materialized_publications.get(&name).cloned(),
            )
        };

        let canonical_publication = publications.publication(&name)?;
        if canonical_publication.surface() != publication.surface() {
            return Err(ServiceMaterializationError::PublicationSurfaceMismatch {
                name,
                publication_surface_id: canonical_publication.surface().surface_id().clone(),
                backed_surface_id: publication.surface().surface_id().clone(),
            });
        }

        let backing = backing.ok_or_else(|| {
            ServiceMaterializationError::UnknownServiceBacking(
                canonical_publication.surface().surface_id().clone(),
            )
        })?;

        if let Some(existing) = cached
            && existing.publication() == &canonical_publication
            && existing.backing() == &backing
        {
            return Ok(existing);
        }

        let route = match ingress.resolve_route(backing.service_id()) {
            Ok(route) => route,
            Err(IngressError::NotFound) => ingress.ensure_route(&IngressRouteTarget::new(
                backing.service_id().clone(),
                backing.protocol(),
            ))?,
            Err(error) => return Err(error.into()),
        };

        let materialized =
            MaterializedServicePublication::new(canonical_publication, backing, route);
        let mut state = self.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        state
            .materialized_publications
            .insert(name, materialized.clone());
        Ok(materialized)
    }

    fn materialized_publication(
        &self,
        name: &NamespaceName,
    ) -> Result<MaterializedServicePublication, ServiceMaterializationError> {
        let state = self.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        state
            .materialized_publications
            .get(name)
            .cloned()
            .ok_or_else(|| {
                ServiceMaterializationError::UnknownMaterializedPublication(name.clone())
            })
    }
}

impl GatewayExposureService for SharedMaterializationState {
    fn entry(&self, name: &NamespaceName) -> Result<GatewayExposure, ServiceMaterializationError> {
        let gateway = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state.gateway.clone().expect("gateway bound")
        };
        let entry = gateway.entry(name)?;
        project_gateway_entry(self, entry)
    }

    fn entries(&self) -> Result<Vec<GatewayExposure>, ServiceMaterializationError> {
        let gateway = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state.gateway.clone().expect("gateway bound")
        };
        gateway
            .entries()
            .into_iter()
            .map(|entry| project_gateway_entry(self, entry))
            .collect()
    }
}

impl LocalGatewayExposureService for SharedMaterializationState {
    fn entry(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
        project_local_gateway_entry(self, name)
    }

    fn entries(&self) -> Result<Vec<LocalGatewayExposure>, ServiceMaterializationError> {
        let gateway = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state.gateway.clone().expect("gateway bound")
        };
        gateway
            .entries()
            .into_iter()
            .map(|entry| project_local_gateway_entry_from_entry(self, entry))
            .collect()
    }

    fn ensure_local_placement(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
        let materialized = self.materialized_publication(name)?;
        let connectivity = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state.connectivity.clone().expect("connectivity bound")
        };
        let _ =
            connectivity.ensure_reachability(&materialized.route().id, ConnectivityScope::Local)?;
        project_local_gateway_entry(self, name)
    }

    fn activate_local_access(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
        let materialized = self.materialized_publication(name)?;
        let connectivity = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state.connectivity.clone().expect("connectivity bound")
        };
        let prepared =
            connectivity.ensure_reachability(&materialized.route().id, ConnectivityScope::Local)?;
        let _ = connectivity.activate_reachability(&prepared.reachability.id)?;
        project_local_gateway_entry(self, name)
    }
}

impl LocalNameResolutionService for SharedMaterializationState {
    fn assign_local_name(
        &self,
        namespace_name: &NamespaceName,
        local_name: LocalNetworkName,
    ) -> Result<LocalNameResolution, ServiceMaterializationError> {
        let exposure = project_local_gateway_entry(self, namespace_name)?;
        let reachability_id = exposure
            .reachability()
            .map(|reachability| reachability.id.clone())
            .ok_or_else(|| {
                ServiceMaterializationError::LocalNameRequiresLocalPlacement(namespace_name.clone())
            })?;
        let projection =
            LocalNameProjection::new(namespace_name.clone(), local_name.clone(), reachability_id);
        let mut state = self.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        match state.local_name_projections.get(&local_name).cloned() {
            Some(existing) if existing == projection => {}
            Some(_) => {
                return Err(
                    ServiceMaterializationError::LocalNetworkNameAlreadyAllocated(local_name),
                );
            }
            None => {
                state
                    .local_name_projections
                    .insert(local_name, projection.clone());
            }
        }
        Ok(LocalNameResolution::new(projection, exposure))
    }

    fn resolve_local_name(
        &self,
        local_name: &LocalNetworkName,
    ) -> Result<LocalNameResolution, ServiceMaterializationError> {
        let projection = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state
                .local_name_projections
                .get(local_name)
                .cloned()
                .ok_or_else(|| {
                    ServiceMaterializationError::UnknownLocalNetworkName(local_name.clone())
                })?
        };
        let exposure = project_local_gateway_entry(self, projection.namespace_name())?;
        Ok(LocalNameResolution::new(projection, exposure))
    }

    fn local_names(&self) -> Result<Vec<LocalNameResolution>, ServiceMaterializationError> {
        let projections = {
            let state = self.inner.lock().expect("materialization state lock");
            ensure_available(&state)?;
            state
                .local_name_projections
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        projections
            .into_iter()
            .map(|projection| {
                let exposure = project_local_gateway_entry(self, projection.namespace_name())?;
                Ok(LocalNameResolution::new(projection, exposure))
            })
            .collect()
    }
}

impl MaterializationState {
    fn instance_id(&self) -> InstanceId {
        self.instance_id.clone().expect("instance id bound")
    }
}

fn ensure_available(state: &MaterializationState) -> Result<(), ServiceMaterializationError> {
    if state.available {
        Ok(())
    } else {
        Err(ServiceMaterializationError::Unavailable)
    }
}

fn ensure_surface_instance(
    instance_id: &InstanceId,
    surface: &Surface,
) -> Result<(), ServiceMaterializationError> {
    let owner_instance_id = surface.owner().instance_id();
    if owner_instance_id == instance_id {
        Ok(())
    } else {
        Err(
            fabric_component::ComponentError::SurfaceOwnerInstanceMismatch {
                surface_id: surface.surface_id().clone(),
                owner_instance_id: owner_instance_id.clone(),
                instance_id: instance_id.clone(),
            }
            .into(),
        )
    }
}

fn project_gateway_entry(
    shared: &SharedMaterializationState,
    entry: GatewayEntry,
) -> Result<GatewayExposure, ServiceMaterializationError> {
    let (service, backing, materialized) = {
        let state = shared.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        (
            state.service.clone().expect("service bound"),
            state
                .service_backings
                .get(entry.surface().surface_id())
                .cloned(),
            state.materialized_publications.get(entry.name()).cloned(),
        )
    };

    let availability = match backing {
        None => GatewayExposureAvailability::PublishedOnly,
        Some(backing) => match materialized {
            None => GatewayExposureAvailability::ServiceBackedUnmaterialized,
            Some(_) => match service.resolve_live_endpoint(backing.service_id()) {
                Ok(_) => GatewayExposureAvailability::Available,
                Err(
                    ServiceError::Unavailable | ServiceError::NoTarget | ServiceError::NotFound,
                ) => GatewayExposureAvailability::MaterializedUnavailable,
                Err(error) => return Err(error.into()),
            },
        },
    };

    Ok(GatewayExposure::new(entry, availability))
}

fn project_local_gateway_entry(
    shared: &SharedMaterializationState,
    name: &NamespaceName,
) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
    let gateway = {
        let state = shared.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        state.gateway.clone().expect("gateway bound")
    };
    let entry = gateway.entry(name)?;
    project_local_gateway_entry_from_entry(shared, entry)
}

fn project_local_gateway_entry_from_entry(
    shared: &SharedMaterializationState,
    entry: GatewayEntry,
) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
    let exposure = project_gateway_entry(shared, entry.clone())?;
    let (connectivity, local_connectivity_access, route_id) = {
        let state = shared.inner.lock().expect("materialization state lock");
        ensure_available(&state)?;
        (
            state.connectivity.clone().expect("connectivity bound"),
            state
                .local_connectivity_access
                .clone()
                .expect("local connectivity access bound"),
            state
                .materialized_publications
                .get(entry.name())
                .map(|materialized| materialized.route().id.clone()),
        )
    };

    let reachability = match route_id {
        Some(route_id) => {
            match connectivity.resolve_reachability(&route_id, ConnectivityScope::Local) {
                Ok(reachability) => Some(reachability),
                Err(fabric_resource_connectivity::ConnectivityError::NotFound) => None,
                Err(error) => return Err(error.into()),
            }
        }
        None => None,
    };

    let local_access = match &reachability {
        Some(reachability) => {
            match local_connectivity_access.resolve_local_access(&reachability.id) {
                Ok(access) => Some(access),
                Err(fabric_resource_connectivity::ConnectivityError::Inactive)
                | Err(fabric_resource_connectivity::ConnectivityError::NotFound) => None,
                Err(error) => return Err(error.into()),
            }
        }
        None => None,
    };

    Ok(LocalGatewayExposure::new(
        exposure,
        reachability,
        local_access,
    ))
}

fn reconcile_existing_backing(
    existing: ServiceBackedSurface,
    service_id: &ServiceId,
    protocol: ServiceProtocol,
) -> Result<ServiceBackedSurface, ServiceMaterializationError> {
    if existing.service_id() == service_id && existing.protocol() == protocol {
        Ok(existing)
    } else if existing.service_id() == service_id {
        Err(ServiceMaterializationError::ServiceProtocolMismatch {
            service_id: service_id.clone(),
            expected: existing.protocol(),
            actual: protocol,
        })
    } else {
        Err(ServiceMaterializationError::ConflictingServiceBacking {
            surface_id: existing.surface().surface_id().clone(),
            existing_service_id: existing.service_id().clone(),
            requested_service_id: service_id.clone(),
        })
    }
}
