use std::collections::BTreeMap;
use std::fmt;

use fabric_projection::ProjectionLeases;

use crate::ProcessError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessSpec {
    pub process_id: ProcessId,
    pub artifact: ProcessArtifact,
    pub entrypoint: ProcessEntrypoint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessArtifact {
    pub reference: String,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessEntrypoint(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedProcess {
    pub process_id: ProcessId,
    pub process: ProcessSpec,
}

#[derive(Default)]
pub struct PreparedProcessEnvironment {
    public: BTreeMap<String, String>,
    leases: ProjectionLeases,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartProcessRequest {
    pub prepared_process: PreparedProcess,
    pub process: ProcessSpec,
    pub environment: PreparedProcessEnvironment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopProcessRequest {
    pub process_instance_id: ProcessInstanceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInstance {
    pub process_instance_id: ProcessInstanceId,
    pub process_id: ProcessId,
    pub status: ProcessInstanceStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessInstanceStatus {
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessInstanceId(String);

impl ProcessId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProcessError> {
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
            Err(ProcessError::invalid_input("invalid process id"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PreparedProcess {
    pub fn new(process: ProcessSpec) -> Self {
        Self {
            process_id: process.process_id.clone(),
            process,
        }
    }

    pub fn matches(&self, process: &ProcessSpec) -> bool {
        &self.process == process
    }
}

impl ProcessEntrypoint {
    pub fn new(value: impl Into<String>) -> Result<Self, ProcessError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 256
            && !value.bytes().any(|byte| byte.is_ascii_control())
            && !value.starts_with('/')
            && !value.contains('\\');
        if valid {
            Ok(Self(value))
        } else {
            Err(ProcessError::invalid_input("invalid process entrypoint"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ProcessInstanceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProcessError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(ProcessError::invalid_input("invalid process instance id"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PreparedProcessEnvironment {
    pub fn insert_public(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ProcessError> {
        let name = name.into();
        if !is_valid_environment_name(&name) {
            return Err(ProcessError::invalid_input(
                "invalid process environment name",
            ));
        }
        self.public.insert(name, value.into());
        Ok(())
    }

    pub fn append_leases(&mut self, leases: ProjectionLeases) {
        self.leases.append(leases);
    }

    pub fn public(&self) -> &BTreeMap<String, String> {
        &self.public
    }

    pub fn is_empty(&self) -> bool {
        self.public.is_empty() && self.leases.is_empty()
    }

    pub fn into_parts(self) -> (BTreeMap<String, String>, ProjectionLeases) {
        (self.public, self.leases)
    }
}

impl Clone for PreparedProcessEnvironment {
    fn clone(&self) -> Self {
        let mut public = BTreeMap::new();
        for (name, value) in &self.public {
            public.insert(name.clone(), value.clone());
        }
        Self {
            public,
            leases: ProjectionLeases::default(),
        }
    }
}

impl fmt::Debug for PreparedProcessEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedProcessEnvironment")
            .field("public", &self.public)
            .field("has_leases", &!self.leases.is_empty())
            .finish()
    }
}

impl PartialEq for PreparedProcessEnvironment {
    fn eq(&self, other: &Self) -> bool {
        self.public == other.public
    }
}

impl Eq for PreparedProcessEnvironment {}

impl fmt::Display for ProcessInstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn is_valid_environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
