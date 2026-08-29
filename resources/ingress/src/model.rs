use fabric_resource_service::{Service, ServiceId, ServiceProtocol};
use sha2::{Digest, Sha256};

use crate::IngressError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngressRoute {
    pub id: IngressRouteId,
    pub target: IngressRouteTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IngressRouteTarget {
    pub service_id: ServiceId,
    pub protocol: ServiceProtocol,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IngressRouteId(String);

impl IngressRoute {
    pub fn for_target(target: &IngressRouteTarget) -> Self {
        Self {
            id: IngressRouteId::for_service(&target.service_id, target.protocol),
            target: target.clone(),
        }
    }
}

impl IngressRouteTarget {
    pub fn new(service_id: ServiceId, protocol: ServiceProtocol) -> Self {
        Self {
            service_id,
            protocol,
        }
    }

    pub fn for_service(service: &Service) -> Self {
        Self {
            service_id: service.id.clone(),
            protocol: service.requirement.protocol,
        }
    }
}

impl IngressRouteId {
    pub fn parse(value: impl Into<String>) -> Result<Self, IngressError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(IngressError::invalid_input("invalid ingress route id"))
        }
    }

    pub fn for_service(service_id: &ServiceId, protocol: ServiceProtocol) -> Self {
        let mut digest = Sha256::new();
        digest.update(service_id.as_str().as_bytes());
        digest.update([0]);
        digest.update(protocol.as_str().as_bytes());
        Self(format!("ing_route_{}", hex::encode(digest.finalize())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
