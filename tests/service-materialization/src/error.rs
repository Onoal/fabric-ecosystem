use std::error::Error;
use std::fmt;

use fabric_component::{ComponentError, SurfaceId};
use fabric_component_gateway::GatewayError;
use fabric_component_namespace::NamespaceName;
use fabric_component_publication::PublicationError;
use fabric_resource_connectivity::ConnectivityError;
use fabric_resource_ingress::IngressError;
use fabric_resource_service::{ServiceError, ServiceId, ServiceProtocol};

use crate::LocalNetworkName;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceMaterializationError {
    Unavailable,
    Component(ComponentError),
    Gateway(GatewayError),
    Publication(Box<PublicationError>),
    Connectivity(ConnectivityError),
    Service(ServiceError),
    Ingress(IngressError),
    InvalidLocalNetworkName(String),
    UnknownLocalNetworkName(LocalNetworkName),
    LocalNetworkNameAlreadyAllocated(LocalNetworkName),
    LocalNameRequiresLocalPlacement(NamespaceName),
    UnknownServiceBacking(SurfaceId),
    UnknownMaterializedPublication(NamespaceName),
    ConflictingServiceBacking {
        surface_id: SurfaceId,
        existing_service_id: ServiceId,
        requested_service_id: ServiceId,
    },
    PublicationSurfaceMismatch {
        name: NamespaceName,
        publication_surface_id: SurfaceId,
        backed_surface_id: SurfaceId,
    },
    ServiceProtocolMismatch {
        service_id: ServiceId,
        expected: ServiceProtocol,
        actual: ServiceProtocol,
    },
}

impl fmt::Display for ServiceMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("service materialization is unavailable"),
            Self::Component(error) => write!(formatter, "{error}"),
            Self::Gateway(error) => write!(formatter, "{error}"),
            Self::Publication(error) => write!(formatter, "{error}"),
            Self::Connectivity(error) => write!(formatter, "{error}"),
            Self::Service(error) => write!(formatter, "{error}"),
            Self::Ingress(error) => write!(formatter, "{error}"),
            Self::InvalidLocalNetworkName(name) => {
                write!(formatter, "local network name `{name}` is invalid")
            }
            Self::UnknownLocalNetworkName(name) => {
                write!(formatter, "local network name `{name}` is not assigned")
            }
            Self::LocalNetworkNameAlreadyAllocated(name) => {
                write!(formatter, "local network name `{name}` is already assigned")
            }
            Self::LocalNameRequiresLocalPlacement(name) => write!(
                formatter,
                "publication `{name}` has no local placement to project into a local network name"
            ),
            Self::UnknownServiceBacking(surface_id) => {
                write!(
                    formatter,
                    "SurfaceId `{surface_id}` has no bound Resource service"
                )
            }
            Self::UnknownMaterializedPublication(name) => {
                write!(formatter, "publication `{name}` is not materialized")
            }
            Self::ConflictingServiceBacking {
                surface_id,
                existing_service_id,
                requested_service_id,
            } => write!(
                formatter,
                "SurfaceId `{surface_id}` is already bound to ServiceId `{existing_service_id}` and cannot be rebound to `{requested_service_id}`"
            ),
            Self::PublicationSurfaceMismatch {
                name,
                publication_surface_id,
                backed_surface_id,
            } => write!(
                formatter,
                "publication `{name}` targets surface `{publication_surface_id}` but the bound Resource service belongs to surface `{backed_surface_id}`"
            ),
            Self::ServiceProtocolMismatch {
                service_id,
                expected,
                actual,
            } => write!(
                formatter,
                "ServiceId `{service_id}` uses protocol `{}` but `{}` was required",
                actual.as_str(),
                expected.as_str()
            ),
        }
    }
}

impl Error for ServiceMaterializationError {}

impl From<ComponentError> for ServiceMaterializationError {
    fn from(value: ComponentError) -> Self {
        Self::Component(value)
    }
}

impl From<ServiceError> for ServiceMaterializationError {
    fn from(value: ServiceError) -> Self {
        Self::Service(value)
    }
}

impl From<GatewayError> for ServiceMaterializationError {
    fn from(value: GatewayError) -> Self {
        Self::Gateway(value)
    }
}

impl From<PublicationError> for ServiceMaterializationError {
    fn from(value: PublicationError) -> Self {
        Self::Publication(Box::new(value))
    }
}

impl From<ConnectivityError> for ServiceMaterializationError {
    fn from(value: ConnectivityError) -> Self {
        Self::Connectivity(value)
    }
}

impl From<IngressError> for ServiceMaterializationError {
    fn from(value: IngressError) -> Self {
        Self::Ingress(value)
    }
}
