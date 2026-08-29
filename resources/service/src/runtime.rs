use std::sync::Arc;

use crate::{ServiceEndpointId, ServiceError, ServiceHttpRequest, ServiceHttpResponse};

pub trait ServiceHttpTargetRuntimeService: Send + Sync {
    fn dispatch_http(
        &self,
        endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError>;
}

#[derive(Clone)]
pub struct ServiceHttpTargetRuntime {
    inner: Arc<dyn ServiceHttpTargetRuntimeService>,
}

impl ServiceHttpTargetRuntime {
    pub fn new(inner: Arc<dyn ServiceHttpTargetRuntimeService>) -> Self {
        Self { inner }
    }

    pub fn dispatch_http(
        &self,
        endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        self.inner.dispatch_http(endpoint_id, request)
    }
}
