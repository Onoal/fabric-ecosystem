use std::fmt;

use crate::ServerError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSpec {
    pub server_id: ServerId,
    pub artifact: ServerArtifact,
    pub entrypoint: ServerEntrypoint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerArtifact {
    pub reference: String,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerEntrypoint(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedServer {
    pub server_id: ServerId,
    pub server: ServerSpec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartServerRequest {
    pub prepared_server: PreparedServer,
    pub server: ServerSpec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopServerRequest {
    pub server_instance_id: ServerInstanceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerInstance {
    pub server_instance_id: ServerInstanceId,
    pub server_id: ServerId,
    pub status: ServerInstanceStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerInstanceStatus {
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerInstanceId(String);

impl ServerId {
    pub fn new(value: impl Into<String>) -> Result<Self, ServerError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && !value.bytes().any(|byte| {
                byte.is_ascii_whitespace()
                    || byte.is_ascii_control()
                    || matches!(byte, b'/' | b'\\')
            })
            && value != "."
            && value != "..";
        if valid {
            Ok(Self(value))
        } else {
            Err(ServerError::invalid_input("invalid server id"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PreparedServer {
    pub fn new(server: ServerSpec) -> Self {
        Self {
            server_id: server.server_id.clone(),
            server,
        }
    }

    pub fn matches(&self, server: &ServerSpec) -> bool {
        &self.server == server
    }
}

impl ServerEntrypoint {
    pub fn new(value: impl Into<String>) -> Result<Self, ServerError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 256
            && !value.bytes().any(|byte| byte.is_ascii_control())
            && !value.starts_with('/')
            && !value.contains('\\');
        if valid {
            Ok(Self(value))
        } else {
            Err(ServerError::invalid_input("invalid server entrypoint"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ServerInstanceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ServerError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(ServerError::invalid_input("invalid server instance id"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
