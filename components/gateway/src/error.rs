use std::error::Error;
use std::fmt;

use fabric_component::{ComponentError, ComponentId, OperationId};
use fabric_component_publication::PublicationError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayError {
    InvalidGatewayStepId(String),
    DuplicateGatewayStepId(crate::GatewayStepId),
    GatewayStepOwnerNotParticipating(crate::GatewayStepId, ComponentId),
    GatewayStepOwnerNotPreparing(crate::GatewayStepId, ComponentId),
    UnknownGatewayOperation(OperationId),
    Publication(Box<PublicationError>),
    Component(ComponentError),
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGatewayStepId(value) => write!(f, "GatewayStepId `{value}` is invalid"),
            Self::DuplicateGatewayStepId(step_id) => {
                write!(f, "GatewayStepId `{step_id}` is already registered")
            }
            Self::GatewayStepOwnerNotParticipating(step_id, component_id) => write!(
                f,
                "GatewayStepId `{step_id}` cannot be registered because ComponentId `{component_id}` is not participating"
            ),
            Self::GatewayStepOwnerNotPreparing(step_id, component_id) => write!(
                f,
                "GatewayStepId `{step_id}` cannot be registered because ComponentId `{component_id}` is no longer preparing"
            ),
            Self::UnknownGatewayOperation(operation_id) => {
                write!(f, "Gateway cannot resolve OperationId `{operation_id}`")
            }
            Self::Publication(error) => error.fmt(f),
            Self::Component(error) => error.fmt(f),
        }
    }
}

impl Error for GatewayError {}

impl From<ComponentError> for GatewayError {
    fn from(value: ComponentError) -> Self {
        Self::Component(value)
    }
}

impl From<PublicationError> for GatewayError {
    fn from(value: PublicationError) -> Self {
        Self::Publication(Box::new(value))
    }
}
