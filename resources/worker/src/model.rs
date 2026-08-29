use semver::Version;
use std::fmt;

use crate::WorkerError;
use crate::bindings::{WorkloadBinding, WorkloadBindingProjection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSpec {
    pub workload_id: WorkloadId,
    pub requirement: WorkloadRequirement,
    pub artifact: WorkloadArtifact,
    pub entrypoint: WorkloadEntrypoint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadArtifact {
    pub reference: String,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadEntrypoint(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadRequirement {
    pub api_version: WorkerApiVersion,
    pub features: Vec<WorkerFeature>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerApiVersion(Version);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkerFeature(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerCapabilities {
    pub api_versions: Vec<WorkerApiVersion>,
    pub features: Vec<WorkerFeatureSupport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerFeatureSupport {
    pub feature: WorkerFeature,
    pub supported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkloadId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedWorker {
    pub workload_id: WorkloadId,
    pub worker: WorkerSpec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartWorkerRequest {
    pub prepared_worker: PreparedWorker,
    pub worker: WorkerSpec,
    pub bindings: Vec<WorkloadBinding>,
    pub binding_projections: Vec<WorkloadBindingProjection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopWorkerRequest {
    pub worker_instance_id: WorkerInstanceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchHttpRequest {
    pub worker_instance_id: WorkerInstanceId,
    pub request: HttpRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerInstance {
    pub worker_instance_id: WorkerInstanceId,
    pub workload_id: WorkloadId,
    pub status: WorkerInstanceStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerInstanceStatus {
    Prepared,
    Running,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkerInstanceId(String);

impl WorkerApiVersion {
    pub fn parse(value: &str) -> Result<Self, WorkerError> {
        Version::parse(value).map(Self).map_err(|error| {
            WorkerError::invalid_input(format!("invalid worker api version: {error}"))
        })
    }

    pub fn as_semver(&self) -> &Version {
        &self.0
    }
}

impl WorkerFeature {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkerError> {
        let value = value.into();
        let valid = value.split('.').count() >= 2
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
            && !value.starts_with('.')
            && !value.ends_with('.')
            && !value.contains("..");
        if valid {
            Ok(Self(value))
        } else {
            Err(WorkerError::invalid_input("invalid worker feature"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WorkloadId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkerError> {
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
            Err(WorkerError::invalid_input("invalid workload id"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PreparedWorker {
    pub fn new(worker: WorkerSpec) -> Self {
        Self {
            workload_id: worker.workload_id.clone(),
            worker,
        }
    }

    pub fn matches(&self, worker: &WorkerSpec) -> bool {
        &self.worker == worker
    }
}

impl WorkloadEntrypoint {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkerError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 256
            && !value.bytes().any(|byte| byte.is_ascii_control())
            && !value.starts_with('/')
            && !value.contains('\\');
        if valid {
            Ok(Self(value))
        } else {
            Err(WorkerError::invalid_input("invalid workload entrypoint"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WorkerInstanceId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkerError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if valid {
            Ok(Self(value))
        } else {
            Err(WorkerError::invalid_input("invalid worker instance id"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl HttpRequest {
    pub fn validate(&self) -> Result<(), WorkerError> {
        validate_http_method(&self.method)?;
        if self.url.is_empty() {
            return Err(WorkerError::invalid_input(
                "http request url must not be empty",
            ));
        }
        for header in &self.headers {
            header.validate()?;
        }
        Ok(())
    }
}

impl HttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, WorkerError> {
        let header = Self {
            name: name.into(),
            value: value.into(),
        };
        header.validate()?;
        Ok(header)
    }

    pub fn validate(&self) -> Result<(), WorkerError> {
        let valid_name = !self.name.is_empty()
            && self.name.len() <= 256
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if !valid_name {
            return Err(WorkerError::invalid_input("invalid http header name"));
        }
        if self
            .value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(WorkerError::invalid_input("invalid http header value"));
        }
        Ok(())
    }
}

impl fmt::Display for WorkerInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

fn validate_http_method(method: &str) -> Result<(), WorkerError> {
    let valid = !method.is_empty()
        && method.len() <= 32
        && method
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(WorkerError::invalid_input("invalid http method"))
    }
}
