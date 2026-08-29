use std::sync::Arc;

use fabric_resource_service::{
    ServiceEndpointId, ServiceError, ServiceHttpRequest, ServiceHttpResponse,
    ServiceHttpTargetRuntimeService,
};
use fabric_resource_worker::{
    DispatchHttpRequest, HttpHeader, HttpRequest, WorkerHttpContract, WorkerInstanceId,
};

#[derive(Clone)]
pub(crate) struct WorkerServiceTargetRuntime {
    worker_http: WorkerHttpContract,
    worker_instance_id: WorkerInstanceId,
    expected_endpoint_id: ServiceEndpointId,
}

impl WorkerServiceTargetRuntime {
    pub(crate) fn shared(
        worker_http: WorkerHttpContract,
        worker_instance_id: WorkerInstanceId,
        expected_endpoint_id: ServiceEndpointId,
    ) -> Arc<dyn ServiceHttpTargetRuntimeService> {
        Arc::new(Self {
            worker_http,
            worker_instance_id,
            expected_endpoint_id,
        })
    }
}

impl ServiceHttpTargetRuntimeService for WorkerServiceTargetRuntime {
    fn dispatch_http(
        &self,
        endpoint_id: &ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        if endpoint_id != &self.expected_endpoint_id {
            return Err(ServiceError::InvalidInput {
                message: "service endpoint does not match worker target runtime".to_owned(),
            });
        }
        request.validate()?;
        let response = self
            .worker_http
            .dispatch_http(DispatchHttpRequest {
                worker_instance_id: self.worker_instance_id.clone(),
                request: HttpRequest {
                    method: request.method,
                    url: request.url,
                    headers: request
                        .headers
                        .into_iter()
                        .map(|header| HttpHeader {
                            name: header.name,
                            value: header.value,
                        })
                        .collect(),
                    body: request.body,
                },
            })
            .map_err(|error| ServiceError::DispatchFailed {
                message: error.to_string(),
            })?;
        Ok(ServiceHttpResponse {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(|header| fabric_resource_service::ServiceHttpHeader {
                    name: header.name,
                    value: header.value,
                })
                .collect(),
            body: response.body,
        })
    }
}
