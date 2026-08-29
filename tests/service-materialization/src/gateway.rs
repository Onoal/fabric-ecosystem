use std::sync::Arc;

use fabric_component_gateway::GatewayEntry;
use fabric_component_namespace::NamespaceName;
use fabric_core::{ContractId, ContractKey};

use crate::ServiceMaterializationError;

const GATEWAY_EXPOSURE_CONTRACT_ID: &str =
    "fabric.composition.service-materialization.gateway-exposure";

pub fn gateway_exposure_contract_id() -> ContractId {
    ContractId::new(GATEWAY_EXPOSURE_CONTRACT_ID).expect("static gateway exposure contract id")
}

pub fn gateway_exposure_contract_key() -> ContractKey<GatewayExposureContract> {
    ContractKey::provisional(gateway_exposure_contract_id())
}

pub trait GatewayExposureService: Send + Sync {
    fn entry(&self, name: &NamespaceName) -> Result<GatewayExposure, ServiceMaterializationError>;

    fn entries(&self) -> Result<Vec<GatewayExposure>, ServiceMaterializationError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayExposureAvailability {
    PublishedOnly,
    ServiceBackedUnmaterialized,
    MaterializedUnavailable,
    Available,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayExposure {
    entry: GatewayEntry,
    availability: GatewayExposureAvailability,
}

#[derive(Clone)]
pub struct GatewayExposureContract {
    inner: Arc<dyn GatewayExposureService>,
}

impl GatewayExposure {
    pub fn new(entry: GatewayEntry, availability: GatewayExposureAvailability) -> Self {
        Self {
            entry,
            availability,
        }
    }

    pub fn entry(&self) -> &GatewayEntry {
        &self.entry
    }

    pub fn availability(&self) -> GatewayExposureAvailability {
        self.availability
    }
}

impl GatewayExposureContract {
    pub fn new(inner: Arc<dyn GatewayExposureService>) -> Self {
        Self { inner }
    }

    pub fn entry(
        &self,
        name: &NamespaceName,
    ) -> Result<GatewayExposure, ServiceMaterializationError> {
        self.inner.entry(name)
    }

    pub fn entries(&self) -> Result<Vec<GatewayExposure>, ServiceMaterializationError> {
        self.inner.entries()
    }
}
