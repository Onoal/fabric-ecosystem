use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, Response, StatusCode};
use axum::routing::any;
use axum::{Router, response::IntoResponse};
use fabric_resource_service::{
    ServiceContract, ServiceError, ServiceHttpHeader, ServiceHttpRequest, ServiceHttpResponse,
    ServiceId,
};
use serde_json::json;

use crate::projection::PreparedWorkloadProjections;
use crate::{BindingProjection, WorkerError, WorkloadBinding, WorkloadBindingProjection};
use fabric_projection::{ProjectionError, ProjectionLeases};

use super::bridge::{bind_loopback_listener, spawn_router};

#[derive(Clone)]
struct ServiceBridgeConfig {
    service: ServiceContract,
    service_id: ServiceId,
}

pub(crate) fn project_service_binding(
    binding: &WorkloadBinding,
    service_id: &ServiceId,
    projections: &[WorkloadBindingProjection],
    service: &ServiceContract,
    prepared: &mut PreparedWorkloadProjections,
) -> Result<(), ProjectionError> {
    let (listener, address) = bind_loopback_listener().map_err(map_projection_error)?;
    let lease = spawn_router(
        listener,
        service_router(ServiceBridgeConfig {
            service: service.clone(),
            service_id: service_id.clone(),
        }),
    )
    .map_err(map_projection_error)?;
    let base_url = format!("http://127.0.0.1:{}", address.port());
    prepared.allow_network_authority(format!("127.0.0.1:{}", address.port()));
    for projection in projections {
        match projection.projection() {
            BindingProjection::Structured => {
                prepared.insert_structured(
                    binding.name().as_str(),
                    json!({
                        "kind": "service",
                        "serviceId": service_id.as_str(),
                        "baseUrl": base_url,
                    }),
                );
            }
            BindingProjection::Environment(variable) => {
                prepared.insert_public_environment(variable.as_str(), base_url.clone());
            }
        }
    }
    let mut leases = ProjectionLeases::default();
    leases.push(lease);
    prepared.append_leases(leases);
    Ok(())
}

fn map_projection_error(error: WorkerError) -> ProjectionError {
    ProjectionError::materialization_failed(error.to_string())
}

fn service_router(config: ServiceBridgeConfig) -> Router {
    Router::new()
        .route("/", any(dispatch_service))
        .route("/{*path}", any(dispatch_service))
        .with_state(config)
}

async fn dispatch_service(
    State(config): State<ServiceBridgeConfig>,
    request: Request,
) -> impl IntoResponse {
    match into_service_request(request).await {
        Ok(service_request) => match config
            .service
            .dispatch_http(&config.service_id, service_request)
        {
            Ok(response) => into_axum_response(response),
            Err(error) => service_error_response(error),
        },
        Err(error) => service_error_response(error),
    }
}

async fn into_service_request(request: Request) -> Result<ServiceHttpRequest, ServiceError> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, usize::MAX)
        .await
        .map_err(|error| ServiceError::DispatchFailed {
            message: format!("read service projection request body: {error}"),
        })?;
    let mut headers = Vec::new();
    for (name, value) in &parts.headers {
        let value = value.to_str().map_err(|error| ServiceError::InvalidInput {
            message: format!("invalid service projection request header value: {error}"),
        })?;
        headers.push(ServiceHttpHeader::new(name.as_str(), value)?);
    }
    let mut url = parts.uri.path().to_owned();
    if let Some(query) = parts.uri.query() {
        url.push('?');
        url.push_str(query);
    }
    Ok(ServiceHttpRequest {
        method: parts.method.as_str().to_owned(),
        url,
        headers,
        body: body.to_vec(),
    })
}

fn into_axum_response(response: ServiceHttpResponse) -> Response<Body> {
    let mut builder = Response::builder().status(response.status);
    for header in response.headers {
        let Ok(name) = HeaderName::from_bytes(header.name.as_bytes()) else {
            return service_error_response(ServiceError::DispatchFailed {
                message: "service response contained invalid header name".to_owned(),
            });
        };
        let Ok(value) = HeaderValue::from_str(&header.value) else {
            return service_error_response(ServiceError::DispatchFailed {
                message: "service response contained invalid header value".to_owned(),
            });
        };
        builder = builder.header(name, value);
    }
    builder
        .body(Body::from(response.body))
        .unwrap_or_else(|error| {
            service_error_response(ServiceError::DispatchFailed {
                message: format!("build service projection response: {error}"),
            })
        })
}

fn service_error_response(error: ServiceError) -> Response<Body> {
    let status = match error {
        ServiceError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
        ServiceError::NotFound => StatusCode::NOT_FOUND,
        ServiceError::Unavailable | ServiceError::NoTarget => StatusCode::SERVICE_UNAVAILABLE,
        ServiceError::DispatchFailed { .. } => StatusCode::BAD_GATEWAY,
        ServiceError::Integrity { .. }
        | ServiceError::Persistence { .. }
        | ServiceError::PublishFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    Response::builder()
        .status(status)
        .body(Body::from(error.to_string()))
        .expect("service error response")
}
