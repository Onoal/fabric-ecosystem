use std::fmt;

use sha2::{Digest, Sha256};

use crate::ServiceError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceRequirement {
    pub name: ServiceName,
    pub protocol: ServiceProtocol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    pub id: ServiceId,
    pub scope: ServiceScope,
    pub requirement: ServiceRequirement,
    pub endpoint: ServiceEndpoint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceLiveEndpoint {
    pub service_id: ServiceId,
    pub target_id: ServiceTargetId,
    pub protocol: ServiceProtocol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedService {
    pub service: Service,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterServiceTargetRequest {
    pub service_id: ServiceId,
    pub target: ServiceTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkServiceTargetReadyRequest {
    pub service_id: ServiceId,
    pub target_id: ServiceTargetId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkServiceTargetDrainingRequest {
    pub service_id: ServiceId,
    pub target_id: ServiceTargetId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithdrawServiceTargetRequest {
    pub service_id: ServiceId,
    pub target_id: ServiceTargetId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<ServiceHttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceHttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceHttpResponse {
    pub status: u16,
    pub headers: Vec<ServiceHttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceEndpoint {
    pub id: ServiceEndpointId,
    pub service_id: ServiceId,
    pub protocol: ServiceProtocol,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceEndpointId(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceName(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceScope(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceProtocol {
    Http,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceTargetId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceTargetState {
    Registered,
    Ready,
    Draining,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceTarget {
    pub id: ServiceTargetId,
    pub endpoint_id: ServiceEndpointId,
    pub state: ServiceTargetState,
}

impl ServiceRequirement {
    pub fn new(protocol: ServiceProtocol, name: impl Into<String>) -> Result<Self, ServiceError> {
        Ok(Self {
            name: ServiceName::new(name)?,
            protocol,
        })
    }
}

impl ServiceHttpRequest {
    pub fn validate(&self) -> Result<(), ServiceError> {
        validate_http_method(&self.method)?;
        if self.url.is_empty() {
            return Err(ServiceError::invalid_input(
                "service http request url must not be empty",
            ));
        }
        for header in &self.headers {
            header.validate()?;
        }
        Ok(())
    }
}

impl ServiceHttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, ServiceError> {
        let header = Self {
            name: name.into(),
            value: value.into(),
        };
        header.validate()?;
        Ok(header)
    }

    pub fn validate(&self) -> Result<(), ServiceError> {
        let valid_name = !self.name.is_empty()
            && self.name.len() <= 256
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if !valid_name {
            return Err(ServiceError::invalid_input(
                "invalid service http header name",
            ));
        }
        if self
            .value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(ServiceError::invalid_input(
                "invalid service http header value",
            ));
        }
        Ok(())
    }
}

impl ServiceId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ServiceError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(ServiceError::invalid_input("invalid service id"))
        }
    }

    pub fn for_scope(scope: &ServiceScope, requirement: &ServiceRequirement) -> Self {
        let mut digest = Sha256::new();
        digest.update(scope.as_str().as_bytes());
        digest.update([0]);
        digest.update(requirement.name.as_str().as_bytes());
        digest.update([0]);
        digest.update(requirement.protocol.as_str().as_bytes());
        Self(format!("svc_{}", hex::encode(digest.finalize())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Service {
    pub fn from_parts(id: ServiceId, scope: ServiceScope, requirement: ServiceRequirement) -> Self {
        let endpoint = ServiceEndpoint::for_service(&id, requirement.protocol);
        Self {
            id,
            scope,
            requirement,
            endpoint,
        }
    }
}

impl ServiceEndpoint {
    pub fn for_service(service_id: &ServiceId, protocol: ServiceProtocol) -> Self {
        Self {
            id: ServiceEndpointId::for_service(service_id, protocol),
            service_id: service_id.clone(),
            protocol,
        }
    }
}

impl ServiceEndpointId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ServiceError> {
        parse_opaque_id(value.into(), "invalid service endpoint id").map(Self)
    }

    pub fn for_service(service_id: &ServiceId, protocol: ServiceProtocol) -> Self {
        let mut digest = Sha256::new();
        digest.update(service_id.as_str().as_bytes());
        digest.update([0]);
        digest.update(protocol.as_str().as_bytes());
        Self(format!("sep_{}", hex::encode(digest.finalize())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ServiceName {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            });
        if valid {
            Ok(Self(value))
        } else {
            Err(ServiceError::invalid_input("invalid service name"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ServiceScope {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 256
            && !value.starts_with('.')
            && !value.ends_with('.')
            && !value.contains("..")
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            });
        if valid {
            Ok(Self(value))
        } else {
            Err(ServiceError::invalid_input("invalid service scope"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ServiceTargetId {
    pub fn new(value: impl Into<String>) -> Result<Self, ServiceError> {
        parse_opaque_id(value.into(), "invalid service target id").map(Self)
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ServiceError> {
        Self::new(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ServiceProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ServiceError> {
        match value {
            "http" => Ok(Self::Http),
            _ => Err(ServiceError::invalid_input("unsupported service protocol")),
        }
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

fn validate_http_method(method: &str) -> Result<(), ServiceError> {
    let valid = !method.is_empty()
        && method.len() <= 32
        && method
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(ServiceError::invalid_input("invalid service http method"))
    }
}

fn parse_opaque_id(value: String, message: &str) -> Result<String, ServiceError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(value)
    } else {
        Err(ServiceError::invalid_input(message))
    }
}
