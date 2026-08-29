#![forbid(unsafe_code)]

mod contract;
mod error;
mod gateway;
mod local;
mod model;
mod naming;
mod native;

#[cfg(test)]
mod tests;

pub use contract::{
    ServiceMaterializationContract, ServiceMaterializationService,
    service_materialization_contract_id, service_materialization_contract_key,
};
pub use error::ServiceMaterializationError;
pub use gateway::{
    GatewayExposure, GatewayExposureAvailability, GatewayExposureContract, GatewayExposureService,
    gateway_exposure_contract_id, gateway_exposure_contract_key,
};
pub use local::{
    LocalGatewayExposure, LocalGatewayExposureContract, LocalGatewayExposureService,
    local_gateway_exposure_contract_id, local_gateway_exposure_contract_key,
};
pub use model::{MaterializedServicePublication, ServiceBackedSurface};
pub use naming::{
    LocalNameProjection, LocalNameResolution, LocalNameResolutionContract,
    LocalNameResolutionService, LocalNetworkName, local_name_resolution_contract_id,
    local_name_resolution_contract_key,
};
pub use native::ServiceMaterializationModule;
