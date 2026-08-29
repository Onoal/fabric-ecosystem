use std::error::Error;
use std::fmt;

use fabric_component::{ComponentError, ComponentId, InstanceId};

use crate::NamespaceName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamespaceError {
    InvalidNamespaceName(String),
    NamespaceNameOwnerInstanceMismatch {
        name: NamespaceName,
        owner_instance_id: InstanceId,
        instance_id: InstanceId,
    },
    NamespaceNameAlreadyAllocated(NamespaceName),
    UnknownNamespaceName(NamespaceName),
    OwnerNotParticipating(ComponentId),
    Unavailable,
    Component(ComponentError),
}

impl fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNamespaceName(value) => write!(f, "NamespaceName `{value}` is invalid"),
            Self::NamespaceNameOwnerInstanceMismatch {
                name,
                owner_instance_id,
                instance_id,
            } => write!(
                f,
                "NamespaceName `{name}` belongs to Instance `{owner_instance_id}` but Namespace owns `{instance_id}`"
            ),
            Self::NamespaceNameAlreadyAllocated(name) => {
                write!(f, "NamespaceName `{name}` is already allocated")
            }
            Self::UnknownNamespaceName(name) => write!(f, "NamespaceName `{name}` is unknown"),
            Self::OwnerNotParticipating(component_id) => write!(
                f,
                "ComponentId `{component_id}` is not participating in Namespace"
            ),
            Self::Unavailable => f.write_str("Namespace is unavailable"),
            Self::Component(error) => error.fmt(f),
        }
    }
}

impl Error for NamespaceError {}

impl From<ComponentError> for NamespaceError {
    fn from(value: ComponentError) -> Self {
        Self::Component(value)
    }
}
