use std::sync::Arc;

use fabric_component::{Surface, SurfaceId};
use fabric_component_namespace::NamespaceName;
use fabric_component_publication::Publication;
use fabric_core::{ContractId, ContractKey};
use fabric_resource_service::{ServiceId, ServiceProtocol};

use crate::{MaterializedServicePublication, ServiceBackedSurface, ServiceMaterializationError};

const SERVICE_MATERIALIZATION_CONTRACT_ID: &str = "fabric.composition.service-materialization";

pub fn service_materialization_contract_id() -> ContractId {
    ContractId::new(SERVICE_MATERIALIZATION_CONTRACT_ID)
        .expect("static service materialization contract id")
}

pub fn service_materialization_contract_key() -> ContractKey<ServiceMaterializationContract> {
    ContractKey::provisional(service_materialization_contract_id())
}

pub trait ServiceMaterializationService: Send + Sync {
    fn bind_service(
        &self,
        surface: Surface,
        service_id: ServiceId,
        protocol: ServiceProtocol,
    ) -> Result<ServiceBackedSurface, ServiceMaterializationError>;

    fn service_backing(
        &self,
        surface_id: &SurfaceId,
    ) -> Result<ServiceBackedSurface, ServiceMaterializationError>;

    fn materialize_publication(
        &self,
        publication: Publication,
    ) -> Result<MaterializedServicePublication, ServiceMaterializationError>;

    fn materialized_publication(
        &self,
        name: &NamespaceName,
    ) -> Result<MaterializedServicePublication, ServiceMaterializationError>;
}

#[derive(Clone)]
pub struct ServiceMaterializationContract {
    inner: Arc<dyn ServiceMaterializationService>,
}

impl ServiceMaterializationContract {
    pub fn new(inner: Arc<dyn ServiceMaterializationService>) -> Self {
        Self { inner }
    }

    pub fn bind_service(
        &self,
        surface: Surface,
        service_id: ServiceId,
        protocol: ServiceProtocol,
    ) -> Result<ServiceBackedSurface, ServiceMaterializationError> {
        self.inner.bind_service(surface, service_id, protocol)
    }

    pub fn service_backing(
        &self,
        surface_id: &SurfaceId,
    ) -> Result<ServiceBackedSurface, ServiceMaterializationError> {
        self.inner.service_backing(surface_id)
    }

    pub fn materialize_publication(
        &self,
        publication: Publication,
    ) -> Result<MaterializedServicePublication, ServiceMaterializationError> {
        self.inner.materialize_publication(publication)
    }

    pub fn materialized_publication(
        &self,
        name: &NamespaceName,
    ) -> Result<MaterializedServicePublication, ServiceMaterializationError> {
        self.inner.materialized_publication(name)
    }
}
