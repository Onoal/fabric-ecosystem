#![forbid(unsafe_code)]

mod contract;
mod error;
mod model;
mod native;
mod step;

pub use contract::{
    Gateway, GatewayFuture, GatewayService, gateway_contract_id, gateway_contract_key,
};
pub use error::GatewayError;
pub use model::{GatewayEntry, GatewayRequest, GatewayResponse};
pub use native::GatewayModule;
pub use step::{
    GatewayStepApplicability, GatewayStepCall, GatewayStepExecution, GatewayStepId,
    GatewayStepPhase, GatewayStepRegistrar, GatewayStepService, gateway_step_registrar_contract_id,
    gateway_step_registrar_contract_key,
};
