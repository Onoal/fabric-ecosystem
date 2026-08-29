use std::sync::Arc;

use fabric_component_namespace::NamespaceName;
use fabric_core::{ContractId, ContractKey};
use fabric_resource_connectivity::{LocalConnectivityAccess, Reachability};

use crate::{GatewayExposure, ServiceMaterializationError};

const LOCAL_GATEWAY_EXPOSURE_CONTRACT_ID: &str =
    "fabric.composition.service-materialization.local-gateway-exposure";

pub fn local_gateway_exposure_contract_id() -> ContractId {
    ContractId::new(LOCAL_GATEWAY_EXPOSURE_CONTRACT_ID)
        .expect("static local gateway exposure contract id")
}

pub fn local_gateway_exposure_contract_key() -> ContractKey<LocalGatewayExposureContract> {
    ContractKey::provisional(local_gateway_exposure_contract_id())
}

pub trait LocalGatewayExposureService: Send + Sync {
    fn entry(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError>;

    fn entries(&self) -> Result<Vec<LocalGatewayExposure>, ServiceMaterializationError>;

    fn ensure_local_placement(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError>;

    fn activate_local_access(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalGatewayExposure {
    exposure: GatewayExposure,
    reachability: Option<Reachability>,
    local_access: Option<LocalConnectivityAccess>,
}

#[derive(Clone)]
pub struct LocalGatewayExposureContract {
    inner: Arc<dyn LocalGatewayExposureService>,
}

impl LocalGatewayExposure {
    pub fn new(
        exposure: GatewayExposure,
        reachability: Option<Reachability>,
        local_access: Option<LocalConnectivityAccess>,
    ) -> Self {
        Self {
            exposure,
            reachability,
            local_access,
        }
    }

    pub fn exposure(&self) -> &GatewayExposure {
        &self.exposure
    }

    pub fn reachability(&self) -> Option<&Reachability> {
        self.reachability.as_ref()
    }

    pub fn local_access(&self) -> Option<&LocalConnectivityAccess> {
        self.local_access.as_ref()
    }
}

impl LocalGatewayExposureContract {
    pub fn new(inner: Arc<dyn LocalGatewayExposureService>) -> Self {
        Self { inner }
    }

    pub fn entry(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
        self.inner.entry(name)
    }

    pub fn entries(&self) -> Result<Vec<LocalGatewayExposure>, ServiceMaterializationError> {
        self.inner.entries()
    }

    pub fn ensure_local_placement(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
        self.inner.ensure_local_placement(name)
    }

    pub fn activate_local_access(
        &self,
        name: &NamespaceName,
    ) -> Result<LocalGatewayExposure, ServiceMaterializationError> {
        self.inner.activate_local_access(name)
    }
}
