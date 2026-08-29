use std::sync::Arc;

use fabric_core::{Health, ModuleError};
use fabric_resource_service::ServiceContract;

use crate::{IngressService, LocalHttpIngressAccessService};

pub trait IngressAdapter: Send + Sync {
    fn ingress_service(&self) -> Arc<dyn IngressService>;

    fn local_http_ingress_access_service(&self) -> Arc<dyn LocalHttpIngressAccessService>;

    fn bind_service(&self, service: ServiceContract);

    fn initialize(&self) -> Result<(), ModuleError>;

    fn start(&self) -> Result<(), ModuleError>;

    fn stop(&self);

    fn health(&self) -> Health;
}
