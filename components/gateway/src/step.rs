use std::fmt;
use std::sync::Arc;

use fabric_component::{Component, ComponentParticipation, InvocationContext, OperationId};
use fabric_core::{ContractId, ContractKey};

use crate::GatewayError;

pub type GatewayStepApplicability = Arc<dyn Fn(&GatewayStepCall) -> bool + Send + Sync>;
pub type GatewayStepExecution =
    Arc<dyn Fn(&GatewayStepCall) -> Result<(), GatewayError> + Send + Sync>;

const GATEWAY_STEP_REGISTRAR_CONTRACT_ID: &str = "fabric.component.gateway.steps";

pub fn gateway_step_registrar_contract_id() -> ContractId {
    ContractId::new(GATEWAY_STEP_REGISTRAR_CONTRACT_ID)
        .expect("static gateway step registrar contract id")
}

pub fn gateway_step_registrar_contract_key() -> ContractKey<GatewayStepRegistrar> {
    ContractKey::provisional(gateway_step_registrar_contract_id())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GatewayStepId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GatewayStepPhase {
    Enrich,
    Guard,
}

#[derive(Clone, Debug)]
pub struct GatewayStepCall {
    context: InvocationContext,
    operation_id: OperationId,
    operation_owner: Component,
}

pub trait GatewayStepService: Send + Sync {
    fn register(
        &self,
        owner: ComponentParticipation,
        step_id: GatewayStepId,
        phase: GatewayStepPhase,
        applies: GatewayStepApplicability,
        execute: GatewayStepExecution,
    ) -> Result<(), GatewayError>;
}

#[derive(Clone)]
pub struct GatewayStepRegistrar {
    inner: Arc<dyn GatewayStepService>,
}

impl GatewayStepId {
    pub fn new(value: impl Into<String>) -> Result<Self, GatewayError> {
        let value = value.into();
        if value.trim() != value
            || value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(GatewayError::InvalidGatewayStepId(value));
        }
        Ok(Self(value))
    }
}

impl fmt::Display for GatewayStepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl GatewayStepCall {
    pub fn new(
        context: InvocationContext,
        operation_id: OperationId,
        operation_owner: Component,
    ) -> Self {
        Self {
            context,
            operation_id,
            operation_owner,
        }
    }

    pub fn context(&self) -> &InvocationContext {
        &self.context
    }

    pub fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    pub fn operation_owner(&self) -> &Component {
        &self.operation_owner
    }
}

impl GatewayStepRegistrar {
    pub fn new(inner: Arc<dyn GatewayStepService>) -> Self {
        Self { inner }
    }

    pub fn register(
        &self,
        owner: ComponentParticipation,
        step_id: GatewayStepId,
        phase: GatewayStepPhase,
        applies: impl Fn(&GatewayStepCall) -> bool + Send + Sync + 'static,
        execute: impl Fn(&GatewayStepCall) -> Result<(), GatewayError> + Send + Sync + 'static,
    ) -> Result<(), GatewayError> {
        self.inner
            .register(owner, step_id, phase, Arc::new(applies), Arc::new(execute))
    }
}
