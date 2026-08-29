use std::sync::Arc;

use async_trait::async_trait;
use fabric_resource_ingress::{IngressError, IngressRouteId};
use fabric_resource_service::{
    ServiceError, ServiceHttpHeader, ServiceHttpRequest, ServiceHttpResponse,
};
use http::{Response, StatusCode};
use pingora::apps::http_app::ServeHttp;
use pingora::protocols::http::ServerSession;

use crate::adapter::shared::SharedPingoraState;

const READY_PATH: &str = "/__fabric_pingora_ready";

pub(crate) struct PingoraHttpApp {
    shared: Arc<SharedPingoraState>,
}

impl PingoraHttpApp {
    pub(crate) fn new(shared: Arc<SharedPingoraState>) -> Self {
        Self { shared }
    }
}

#[async_trait]
impl ServeHttp for PingoraHttpApp {
    async fn response(&self, http_session: &mut ServerSession) -> Response<Vec<u8>> {
        dispatch_request(&self.shared, http_session)
            .await
            .unwrap_or_else(map_error_response)
    }
}

async fn dispatch_request(
    shared: &SharedPingoraState,
    http_session: &mut ServerSession,
) -> Result<Response<Vec<u8>>, IngressError> {
    let raw_target = render_raw_target(http_session.req_header())?;
    if raw_target == READY_PATH {
        return Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Vec::new())
            .expect("ready response"));
    }

    let (route_segment, service_target) = split_route_target(&raw_target)?;
    let route_id = IngressRouteId::parse(route_segment)?;
    let (service, route_service_id) = {
        let state = shared.inner.lock().expect("pingora ingress lock");
        ensure_started(&state)?;
        let route = state.routes.get(&route_id).ok_or(IngressError::NotFound)?;
        let _access = state
            .local_http_access
            .get(&route_id)
            .ok_or(IngressError::NotFound)?;
        let service = state.service.clone().ok_or(IngressError::Unavailable)?;
        (service, route.target.service_id.clone())
    };

    let body = read_request_body(http_session).await?;
    let request = ServiceHttpRequest {
        method: http_session.req_header().method.as_str().to_owned(),
        url: format!("http://service.local{service_target}"),
        headers: translate_headers(http_session.req_header())?,
        body,
    };
    let response = service
        .dispatch_http(&route_service_id, request)
        .map_err(map_service_error)?;
    Ok(translate_response(response))
}

fn ensure_started(state: &crate::adapter::shared::PingoraState) -> Result<(), IngressError> {
    if state.started {
        Ok(())
    } else {
        Err(IngressError::Unavailable)
    }
}

fn render_raw_target(request: &pingora::http::RequestHeader) -> Result<String, IngressError> {
    request
        .uri
        .path_and_query()
        .map(|value| value.as_str().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| IngressError::DispatchFailed {
            message: "request target is missing".to_owned(),
        })
}

fn split_route_target(path_and_query: &str) -> Result<(String, String), IngressError> {
    let path = path_and_query
        .strip_prefix('/')
        .ok_or_else(|| IngressError::InvalidInput {
            message: "request target must start with '/'".to_owned(),
        })?;
    let separator = path.find(['/', '?']).unwrap_or(path.len());
    let route = &path[..separator];
    if route.is_empty() {
        return Err(IngressError::InvalidInput {
            message: "missing ingress route id".to_owned(),
        });
    }
    let suffix = &path[separator..];
    let stripped = if suffix.is_empty() { "/" } else { suffix };
    Ok((route.to_owned(), stripped.to_owned()))
}

async fn read_request_body(http_session: &mut ServerSession) -> Result<Vec<u8>, IngressError> {
    let mut body = Vec::new();
    while !http_session.is_body_done() {
        match http_session
            .read_body_or_idle(false)
            .await
            .map_err(|error| IngressError::DispatchFailed {
                message: format!("read request body: {error}"),
            })? {
            Some(chunk) => body.extend_from_slice(&chunk),
            None => break,
        }
    }
    Ok(body)
}

fn translate_headers(
    request: &pingora::http::RequestHeader,
) -> Result<Vec<ServiceHttpHeader>, IngressError> {
    request
        .headers
        .iter()
        .map(|(name, value)| {
            ServiceHttpHeader::new(
                name.as_str().to_owned(),
                value
                    .to_str()
                    .map_err(|error| IngressError::DispatchFailed {
                        message: format!("request header is not valid utf-8: {error}"),
                    })?,
            )
            .map_err(|error| IngressError::DispatchFailed {
                message: error.to_string(),
            })
        })
        .collect()
}

fn translate_response(response: ServiceHttpResponse) -> Response<Vec<u8>> {
    let mut builder = Response::builder().status(response.status);
    if let Some(headers) = builder.headers_mut() {
        for header in response.headers {
            if let (Ok(name), Ok(value)) = (
                http::header::HeaderName::try_from(header.name),
                http::header::HeaderValue::try_from(header.value),
            ) {
                headers.append(name, value);
            }
        }
    }
    builder
        .body(response.body)
        .expect("service response builder")
}

fn map_error_response(error: IngressError) -> Response<Vec<u8>> {
    let status = match error {
        IngressError::NotFound => StatusCode::NOT_FOUND,
        IngressError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        IngressError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
        IngressError::ListenerFailed { .. } | IngressError::DispatchFailed { .. } => {
            StatusCode::BAD_GATEWAY
        }
    };
    Response::builder()
        .status(status)
        .body(error.to_string().into_bytes())
        .expect("error response builder")
}

fn map_service_error(error: ServiceError) -> IngressError {
    match error {
        ServiceError::Unavailable => IngressError::Unavailable,
        ServiceError::NotFound => IngressError::NotFound,
        ServiceError::NoTarget => IngressError::Unavailable,
        ServiceError::InvalidInput { message }
        | ServiceError::Integrity { message }
        | ServiceError::Persistence { message }
        | ServiceError::PublishFailed { message }
        | ServiceError::DispatchFailed { message } => IngressError::DispatchFailed { message },
    }
}
