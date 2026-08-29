use std::fmt;
use std::sync::Arc;

use fabric_component_namespace::NamespaceName;
use fabric_core::{ContractId, ContractKey};
use fabric_resource_connectivity::ReachabilityId;

use crate::{LocalGatewayExposure, ServiceMaterializationError};

const LOCAL_NAME_RESOLUTION_CONTRACT_ID: &str =
    "fabric.composition.service-materialization.local-name-resolution";

pub fn local_name_resolution_contract_id() -> ContractId {
    ContractId::new(LOCAL_NAME_RESOLUTION_CONTRACT_ID)
        .expect("static local name resolution contract id")
}

pub fn local_name_resolution_contract_key() -> ContractKey<LocalNameResolutionContract> {
    ContractKey::provisional(local_name_resolution_contract_id())
}

pub trait LocalNameResolutionService: Send + Sync {
    fn assign_local_name(
        &self,
        namespace_name: &NamespaceName,
        local_name: LocalNetworkName,
    ) -> Result<LocalNameResolution, ServiceMaterializationError>;

    fn resolve_local_name(
        &self,
        local_name: &LocalNetworkName,
    ) -> Result<LocalNameResolution, ServiceMaterializationError>;

    fn local_names(&self) -> Result<Vec<LocalNameResolution>, ServiceMaterializationError>;
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalNetworkName(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalNameProjection {
    namespace_name: NamespaceName,
    local_name: LocalNetworkName,
    reachability_id: ReachabilityId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalNameResolution {
    projection: LocalNameProjection,
    exposure: LocalGatewayExposure,
}

#[derive(Clone)]
pub struct LocalNameResolutionContract {
    inner: Arc<dyn LocalNameResolutionService>,
}

impl LocalNetworkName {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceMaterializationError> {
        let value = value.into();
        validate_local_network_name(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl LocalNameProjection {
    pub fn new(
        namespace_name: NamespaceName,
        local_name: LocalNetworkName,
        reachability_id: ReachabilityId,
    ) -> Self {
        Self {
            namespace_name,
            local_name,
            reachability_id,
        }
    }

    pub fn namespace_name(&self) -> &NamespaceName {
        &self.namespace_name
    }

    pub fn local_name(&self) -> &LocalNetworkName {
        &self.local_name
    }

    pub fn reachability_id(&self) -> &ReachabilityId {
        &self.reachability_id
    }
}

impl LocalNameResolution {
    pub fn new(projection: LocalNameProjection, exposure: LocalGatewayExposure) -> Self {
        Self {
            projection,
            exposure,
        }
    }

    pub fn projection(&self) -> &LocalNameProjection {
        &self.projection
    }

    pub fn exposure(&self) -> &LocalGatewayExposure {
        &self.exposure
    }
}

impl LocalNameResolutionContract {
    pub fn new(inner: Arc<dyn LocalNameResolutionService>) -> Self {
        Self { inner }
    }

    pub fn assign_local_name(
        &self,
        namespace_name: &NamespaceName,
        local_name: LocalNetworkName,
    ) -> Result<LocalNameResolution, ServiceMaterializationError> {
        self.inner.assign_local_name(namespace_name, local_name)
    }

    pub fn resolve_local_name(
        &self,
        local_name: &LocalNetworkName,
    ) -> Result<LocalNameResolution, ServiceMaterializationError> {
        self.inner.resolve_local_name(local_name)
    }

    pub fn local_names(&self) -> Result<Vec<LocalNameResolution>, ServiceMaterializationError> {
        self.inner.local_names()
    }
}

impl fmt::Display for LocalNetworkName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn validate_local_network_name(value: &str) -> Result<(), ServiceMaterializationError> {
    if value.trim() != value || value.is_empty() {
        return Err(ServiceMaterializationError::InvalidLocalNetworkName(
            value.to_owned(),
        ));
    }
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return Ok(());
    }
    Err(ServiceMaterializationError::InvalidLocalNetworkName(
        value.to_owned(),
    ))
}
