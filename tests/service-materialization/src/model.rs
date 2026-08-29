use fabric_component::Surface;
use fabric_component_publication::Publication;
use fabric_resource_ingress::IngressRoute;
use fabric_resource_service::{ServiceId, ServiceProtocol};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceBackedSurface {
    surface: Surface,
    service_id: ServiceId,
    protocol: ServiceProtocol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializedServicePublication {
    publication: Publication,
    backing: ServiceBackedSurface,
    route: IngressRoute,
}

impl ServiceBackedSurface {
    pub fn new(surface: Surface, service_id: ServiceId, protocol: ServiceProtocol) -> Self {
        Self {
            surface,
            service_id,
            protocol,
        }
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    pub fn service_id(&self) -> &ServiceId {
        &self.service_id
    }

    pub fn protocol(&self) -> ServiceProtocol {
        self.protocol
    }
}

impl MaterializedServicePublication {
    pub fn new(
        publication: Publication,
        backing: ServiceBackedSurface,
        route: IngressRoute,
    ) -> Self {
        Self {
            publication,
            backing,
            route,
        }
    }

    pub fn publication(&self) -> &Publication {
        &self.publication
    }

    pub fn backing(&self) -> &ServiceBackedSurface {
        &self.backing
    }

    pub fn route(&self) -> &IngressRoute {
        &self.route
    }
}
