use std::error::Error;
use std::fmt;

use fabric_component::{ComponentError, SurfaceId};
use fabric_component_namespace::{NamespaceError, NamespaceName};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationError {
    NamespaceNameAlreadyPublished(NamespaceName),
    UnknownPublication(NamespaceName),
    UnknownSurface(SurfaceId),
    NamespaceClaimMismatch(NamespaceName),
    Namespace(NamespaceError),
    Component(ComponentError),
    Unavailable,
}

impl fmt::Display for PublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamespaceNameAlreadyPublished(name) => {
                write!(f, "NamespaceName `{name}` is already published")
            }
            Self::UnknownPublication(name) => write!(f, "Publication `{name}` is unknown"),
            Self::UnknownSurface(surface_id) => write!(f, "SurfaceId `{surface_id}` is unknown"),
            Self::NamespaceClaimMismatch(name) => {
                write!(f, "Namespace claim `{name}` is no longer current")
            }
            Self::Namespace(error) => error.fmt(f),
            Self::Component(error) => error.fmt(f),
            Self::Unavailable => f.write_str("Publication is unavailable"),
        }
    }
}

impl Error for PublicationError {}

impl From<ComponentError> for PublicationError {
    fn from(value: ComponentError) -> Self {
        Self::Component(value)
    }
}

impl From<NamespaceError> for PublicationError {
    fn from(value: NamespaceError) -> Self {
        Self::Namespace(value)
    }
}
