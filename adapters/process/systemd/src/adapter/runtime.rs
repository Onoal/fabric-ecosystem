use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use fabric_projection::ProjectionLeases;
use fabric_resource_process::{
    ProcessArtifact, ProcessEntrypoint, ProcessError, ProcessExecution, ProcessExecutionRequest,
    ProcessId, ProcessInstanceId, ProcessSpec,
};
use sha2::{Digest, Sha256};

use crate::artifact::ProcessArtifactResolver;
use crate::config::SystemdProcessConfig;

#[allow(dead_code)]
pub(crate) struct StartUnitRequest<'a> {
    pub unit_name: &'a str,
    pub executable: &'a Path,
    pub working_directory: &'a Path,
    pub environment: &'a BTreeMap<String, String>,
}

pub(crate) trait ProcessSupervisor: Send + Sync {
    fn start_transient_unit(&self, request: &StartUnitRequest<'_>) -> Result<(), ProcessError>;
    fn stop_unit(&self, unit_name: &str) -> Result<(), ProcessError>;
    fn active_state(&self, unit_name: &str) -> Result<Option<String>, ProcessError>;
    fn reset_failed(&self, unit_name: &str) -> Result<(), ProcessError>;
}

pub(crate) struct ProcessRuntime {
    config: SystemdProcessConfig,
    resolver: Arc<dyn ProcessArtifactResolver>,
    supervisor: Arc<dyn ProcessSupervisor>,
    prepared: BTreeMap<ProcessId, PreparedProcessArtifact>,
}

#[derive(Clone)]
struct PreparedProcessArtifact {
    execution_root: PathBuf,
    entrypoint_relative: PathBuf,
}

pub(crate) struct SystemdProcessExecution {
    unit_name: String,
    runtime_dir: PathBuf,
    supervisor: Arc<dyn ProcessSupervisor>,
    stop_timeout: Duration,
    projection_leases: ProjectionLeases,
    stopped: bool,
}

impl ProcessRuntime {
    pub(crate) fn new(
        config: SystemdProcessConfig,
        resolver: Arc<dyn ProcessArtifactResolver>,
        supervisor: Arc<dyn ProcessSupervisor>,
    ) -> Self {
        Self {
            config,
            resolver,
            supervisor,
            prepared: BTreeMap::new(),
        }
    }

    pub(crate) fn prepare_process(&mut self, process: &ProcessSpec) -> Result<(), ProcessError> {
        let prepared = self.resolve_and_verify(&process.artifact, &process.entrypoint)?;
        self.prepared.insert(process.process_id.clone(), prepared);
        Ok(())
    }

    pub(crate) fn start_execution(
        &mut self,
        request: ProcessExecutionRequest,
    ) -> Result<Box<dyn ProcessExecution>, ProcessError> {
        let prepared = self
            .prepared
            .get(&request.start.process.process_id)
            .ok_or_else(|| ProcessError::ProtocolViolation {
                message: "process was not prepared by this process runtime".to_owned(),
            })?
            .clone();

        let runtime_dir = self.config.runtime_root.join(request.instance_id.as_str());
        reset_runtime_dir(&runtime_dir)?;
        let staged_root = stage_artifact(&prepared.execution_root, &runtime_dir)?;
        let staged_entry = canonical_file(
            &staged_root.join(&prepared.entrypoint_relative),
            "staged process entrypoint",
        )?;
        let (environment, projection_leases) = request.start.environment.into_parts();
        let unit_name = systemd_unit_name(&request.instance_id);
        let start_result = self.supervisor.start_transient_unit(&StartUnitRequest {
            unit_name: &unit_name,
            executable: &staged_entry,
            working_directory: &staged_root,
            environment: &environment,
        });
        if let Err(error) = start_result {
            let mut projection_leases = projection_leases;
            projection_leases.release_all();
            let _ = fs::remove_dir_all(&runtime_dir);
            return Err(error);
        }
        if let Err(error) = wait_for_active(
            self.supervisor.as_ref(),
            &unit_name,
            self.config.startup_timeout,
        ) {
            let _ = self.supervisor.stop_unit(&unit_name);
            let _ = wait_for_inactive(
                self.supervisor.as_ref(),
                &unit_name,
                self.config.stop_timeout,
            );
            let _ = self.supervisor.reset_failed(&unit_name);
            let mut projection_leases = projection_leases;
            projection_leases.release_all();
            let _ = fs::remove_dir_all(&runtime_dir);
            return Err(error);
        }
        Ok(Box::new(SystemdProcessExecution {
            unit_name,
            runtime_dir,
            supervisor: Arc::clone(&self.supervisor),
            stop_timeout: self.config.stop_timeout,
            projection_leases,
            stopped: false,
        }))
    }

    pub(crate) fn clear(&mut self) {
        self.prepared.clear();
    }

    fn resolve_and_verify(
        &self,
        artifact: &ProcessArtifact,
        entrypoint: &ProcessEntrypoint,
    ) -> Result<PreparedProcessArtifact, ProcessError> {
        let resolved = self.resolver.resolve(artifact)?;
        let artifact_file = canonical_file(&resolved.artifact_file, "artifact")?;
        let execution_root = canonical_directory(&resolved.execution_root, "execution root")?;
        let entrypoint_relative = PathBuf::from(entrypoint.as_str());
        let executable = canonical_file(
            &execution_root.join(&entrypoint_relative),
            "process entrypoint",
        )?;
        if !executable.starts_with(&execution_root) {
            return Err(ProcessError::PrepareFailed {
                message: "process entrypoint is outside the trusted execution root".to_owned(),
            });
        }
        verify_artifact_digest(&artifact_file, artifact)?;
        verify_executable(&executable)?;
        Ok(PreparedProcessArtifact {
            execution_root,
            entrypoint_relative,
        })
    }
}

impl ProcessExecution for SystemdProcessExecution {
    fn stop(&mut self) -> Result<(), ProcessError> {
        self.supervisor.stop_unit(&self.unit_name)?;
        wait_for_inactive(self.supervisor.as_ref(), &self.unit_name, self.stop_timeout)?;
        self.stopped = true;
        Ok(())
    }

    fn cleanup(&mut self) {
        if !self.stopped {
            let _ = self.supervisor.stop_unit(&self.unit_name);
            let _ = wait_for_inactive(self.supervisor.as_ref(), &self.unit_name, self.stop_timeout);
        }
        let _ = self.supervisor.reset_failed(&self.unit_name);
        self.projection_leases.release_all();
        let _ = fs::remove_dir_all(&self.runtime_dir);
        self.stopped = true;
    }
}

fn wait_for_active(
    supervisor: &dyn ProcessSupervisor,
    unit_name: &str,
    timeout: Duration,
) -> Result<(), ProcessError> {
    let deadline = Instant::now() + timeout;
    loop {
        match supervisor.active_state(unit_name)? {
            Some(state) if state == "active" => return Ok(()),
            Some(state) if matches!(state.as_str(), "failed" | "inactive" | "deactivating") => {
                return Err(ProcessError::StartFailed {
                    message: format!(
                        "process transient unit {unit_name} entered terminal state {state}"
                    ),
                });
            }
            _ if Instant::now() >= deadline => {
                return Err(ProcessError::StartFailed {
                    message: format!(
                        "timed out waiting for process transient unit {unit_name} to become active"
                    ),
                });
            }
            _ => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn wait_for_inactive(
    supervisor: &dyn ProcessSupervisor,
    unit_name: &str,
    timeout: Duration,
) -> Result<(), ProcessError> {
    let deadline = Instant::now() + timeout;
    loop {
        match supervisor.active_state(unit_name)? {
            None => return Ok(()),
            Some(state) if state == "inactive" => return Ok(()),
            Some(state) if state == "failed" && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            Some(state) if Instant::now() >= deadline => {
                return Err(ProcessError::StopFailed {
                    message: format!(
                        "timed out waiting for process transient unit {unit_name} to stop from state {state}"
                    ),
                });
            }
            _ => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn verify_artifact_digest(
    artifact_file: &Path,
    artifact: &ProcessArtifact,
) -> Result<(), ProcessError> {
    let actual =
        Sha256::digest(
            fs::read(artifact_file).map_err(|error| ProcessError::PrepareFailed {
                message: format!("read materialized process artifact: {error}"),
            })?,
        );
    if actual.as_slice() != artifact.sha256 {
        return Err(ProcessError::Integrity {
            message: "materialized process artifact digest does not match process artifact"
                .to_owned(),
        });
    }
    Ok(())
}

fn verify_executable(executable: &Path) -> Result<(), ProcessError> {
    let metadata = fs::metadata(executable).map_err(|error| ProcessError::PrepareFailed {
        message: format!("inspect process entrypoint: {error}"),
    })?;
    if !metadata.is_file() {
        return Err(ProcessError::PrepareFailed {
            message: "process entrypoint is not a file".to_owned(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(ProcessError::PrepareFailed {
                message: "process entrypoint is not executable".to_owned(),
            });
        }
    }
    Ok(())
}

fn reset_runtime_dir(runtime_dir: &Path) -> Result<(), ProcessError> {
    if runtime_dir.exists() {
        fs::remove_dir_all(runtime_dir).map_err(|error| ProcessError::StartFailed {
            message: format!("reset process runtime dir: {error}"),
        })?;
    }
    fs::create_dir_all(runtime_dir).map_err(|error| ProcessError::StartFailed {
        message: format!("create process runtime dir: {error}"),
    })?;
    Ok(())
}

fn stage_artifact(execution_root: &Path, runtime_dir: &Path) -> Result<PathBuf, ProcessError> {
    let staged_root = runtime_dir.join("artifact");
    copy_dir_recursive(execution_root, &staged_root)?;
    Ok(staged_root)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), ProcessError> {
    fs::create_dir_all(target).map_err(|error| ProcessError::StartFailed {
        message: format!("create staged process artifact dir: {error}"),
    })?;
    for entry in fs::read_dir(source).map_err(|error| ProcessError::StartFailed {
        message: format!("read process artifact dir: {error}"),
    })? {
        let entry = entry.map_err(|error| ProcessError::StartFailed {
            message: format!("read process artifact entry: {error}"),
        })?;
        let entry_type = entry
            .file_type()
            .map_err(|error| ProcessError::StartFailed {
                message: format!("inspect process artifact entry: {error}"),
            })?;
        let destination = target.join(entry.file_name());
        if entry_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination)?;
        } else if entry_type.is_file() {
            fs::copy(entry.path(), &destination).map_err(|error| ProcessError::StartFailed {
                message: format!("copy process artifact file: {error}"),
            })?;
            copy_permissions(&entry.path(), &destination)?;
        }
    }
    Ok(())
}

fn copy_permissions(source: &Path, destination: &Path) -> Result<(), ProcessError> {
    let permissions = fs::metadata(source)
        .map_err(|error| ProcessError::StartFailed {
            message: format!("read process artifact permissions: {error}"),
        })?
        .permissions();
    fs::set_permissions(destination, permissions).map_err(|error| ProcessError::StartFailed {
        message: format!("apply staged process artifact permissions: {error}"),
    })?;
    Ok(())
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, ProcessError> {
    let canonical = fs::canonicalize(path).map_err(|error| ProcessError::PrepareFailed {
        message: format!("canonicalize {label}: {error}"),
    })?;
    if canonical.is_file() {
        Ok(canonical)
    } else {
        Err(ProcessError::PrepareFailed {
            message: format!("{label} is not a file"),
        })
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ProcessError> {
    let canonical = fs::canonicalize(path).map_err(|error| ProcessError::PrepareFailed {
        message: format!("canonicalize {label}: {error}"),
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(ProcessError::PrepareFailed {
            message: format!("{label} is not a directory"),
        })
    }
}

pub(crate) fn systemd_unit_name(instance_id: &ProcessInstanceId) -> String {
    format!("fabric-resource-process-{}.service", instance_id.as_str())
}
