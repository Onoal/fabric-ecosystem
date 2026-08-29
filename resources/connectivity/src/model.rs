use std::fmt;

use fabric_resource_ingress::IngressRouteId;
use sha2::{Digest, Sha256};

use crate::ConnectivityError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reachability {
    pub id: ReachabilityId,
    pub ingress_route_id: IngressRouteId,
    pub scope: ConnectivityScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedReachability {
    pub reachability: Reachability,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReachabilityId(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectivityScope {
    Local,
}

impl Reachability {
    pub fn new(ingress_route_id: IngressRouteId, scope: ConnectivityScope) -> Self {
        Self {
            id: ReachabilityId::for_route_scope(&ingress_route_id, scope),
            ingress_route_id,
            scope,
        }
    }
}

impl ReachabilityId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ConnectivityError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(ConnectivityError::invalid_input("invalid reachability id"))
        }
    }

    pub fn for_route_scope(ingress_route_id: &IngressRouteId, scope: ConnectivityScope) -> Self {
        let mut digest = Sha256::new();
        digest.update(ingress_route_id.as_str().as_bytes());
        digest.update([0]);
        digest.update(scope.as_str().as_bytes());
        Self(format!("conn_reach_{}", hex::encode(digest.finalize())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ConnectivityScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ConnectivityError> {
        match value {
            "local" => Ok(Self::Local),
            _ => Err(ConnectivityError::invalid_input(
                "unsupported connectivity scope",
            )),
        }
    }
}

impl fmt::Display for ReachabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
