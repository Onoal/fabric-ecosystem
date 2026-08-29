use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fabric_host::{
    HostArchitecture, HostCompatibilityError, HostDescriptor, HostFacilityId, HostOperatingSystem,
};
use fabric_projection::{ProjectionLease, ProjectionLeases};
use fabric_resource_process::{
    PreparedProcess, PreparedProcessEnvironment, ProcessAdapter, ProcessArtifact,
    ProcessEntrypoint, ProcessError, ProcessExecutionRequest, ProcessId, ProcessInstanceId,
    ProcessSpec, StartProcessRequest,
};
use sha2::{Digest, Sha256};

use crate::adapter::{ProcessSupervisor, StartUnitRequest, systemd_unit_name};
use crate::artifact::{ProcessArtifactResolver, ResolvedProcessArtifact};
use crate::config::SystemdProcessConfig;
use crate::{SystemdProcessAdapter, SystemdProcessAdapterError};

#[derive(Clone)]
struct FixtureResolver {
    artifact_file: PathBuf,
    execution_root: PathBuf,
}

impl ProcessArtifactResolver for FixtureResolver {
    fn resolve(
        &self,
        _artifact: &ProcessArtifact,
    ) -> Result<ResolvedProcessArtifact, ProcessError> {
        Ok(ResolvedProcessArtifact {
            artifact_file: self.artifact_file.clone(),
            execution_root: self.execution_root.clone(),
        })
    }
}

#[derive(Default)]
struct SupervisorState {
    start_requests: Vec<RecordedStart>,
    stop_requests: Vec<String>,
    reset_requests: Vec<String>,
    states: BTreeMap<String, Option<String>>,
    fail_start: Option<String>,
}

#[derive(Clone, Debug)]
struct RecordedStart {
    unit_name: String,
    executable: PathBuf,
    working_directory: PathBuf,
    environment: BTreeMap<String, String>,
}

#[derive(Clone)]
struct TestSupervisor {
    state: Arc<Mutex<SupervisorState>>,
}

impl ProcessSupervisor for TestSupervisor {
    fn start_transient_unit(&self, request: &StartUnitRequest<'_>) -> Result<(), ProcessError> {
        let mut state = self.state.lock().expect("supervisor state");
        state.start_requests.push(RecordedStart {
            unit_name: request.unit_name.to_owned(),
            executable: request.executable.to_path_buf(),
            working_directory: request.working_directory.to_path_buf(),
            environment: request.environment.clone(),
        });
        if let Some(message) = state.fail_start.clone() {
            return Err(ProcessError::StartFailed { message });
        }
        state
            .states
            .insert(request.unit_name.to_owned(), Some("active".to_owned()));
        Ok(())
    }

    fn stop_unit(&self, unit_name: &str) -> Result<(), ProcessError> {
        let mut state = self.state.lock().expect("supervisor state");
        state.stop_requests.push(unit_name.to_owned());
        state
            .states
            .insert(unit_name.to_owned(), Some("inactive".to_owned()));
        Ok(())
    }

    fn active_state(&self, unit_name: &str) -> Result<Option<String>, ProcessError> {
        Ok(self
            .state
            .lock()
            .expect("supervisor state")
            .states
            .get(unit_name)
            .cloned()
            .flatten())
    }

    fn reset_failed(&self, unit_name: &str) -> Result<(), ProcessError> {
        self.state
            .lock()
            .expect("supervisor state")
            .reset_requests
            .push(unit_name.to_owned());
        Ok(())
    }
}

struct TestLease(Arc<Mutex<bool>>);

impl ProjectionLease for TestLease {
    fn release(self: Box<Self>) {
        *self.0.lock().expect("released") = true;
    }
}

#[test]
fn systemd_process_adapter_declares_linux_user_systemd_host_requirement() {
    let requirement = SystemdProcessAdapter::host_requirement();

    assert_eq!(requirement.operating_systems().len(), 1);
    assert!(requirement.architectures().is_empty());
    assert_eq!(requirement.required_facilities().len(), 1);
    assert!(
        requirement
            .operating_systems()
            .iter()
            .any(|operating_system| operating_system.as_str() == "linux")
    );
    assert!(
        requirement
            .required_facilities()
            .iter()
            .any(|facility| facility.as_str() == "fabric.host.user-systemd-supervision")
    );
}

#[test]
fn systemd_process_adapter_rejects_non_linux_host_before_runtime_construction() {
    let (_tempdir, resolver, config) = process_fixture();
    let host = HostDescriptor::new(
        HostOperatingSystem::new("macos").expect("os"),
        HostArchitecture::new("aarch64").expect("arch"),
    );

    match SystemdProcessAdapter::for_host(&host, config, resolver) {
        Err(SystemdProcessAdapterError::IncompatibleHost {
            requirement,
            source,
        }) => {
            match source.as_ref() {
                HostCompatibilityError::UnsupportedOperatingSystem { actual, allowed } => {
                    assert_eq!(actual.as_str(), "macos");
                    assert!(
                        allowed
                            .iter()
                            .any(|operating_system| operating_system.as_str() == "linux")
                    );
                }
                other => panic!("unexpected host compatibility source: {other:?}"),
            }
            assert!(
                requirement
                    .required_facilities()
                    .iter()
                    .any(|facility| facility.as_str() == "fabric.host.user-systemd-supervision")
            );
        }
        _ => panic!("unexpected incompatible host result"),
    }
}

#[test]
fn systemd_process_adapter_rejects_linux_host_without_required_facility() {
    let (_tempdir, resolver, config) = process_fixture();
    let host = HostDescriptor::new(
        HostOperatingSystem::new("linux").expect("os"),
        HostArchitecture::new("x86_64").expect("arch"),
    );

    match SystemdProcessAdapter::for_host(&host, config, resolver) {
        Err(SystemdProcessAdapterError::IncompatibleHost {
            requirement,
            source,
        }) => {
            match source.as_ref() {
                HostCompatibilityError::MissingRequiredFacilities { missing } => {
                    assert!(missing.iter().any(
                        |facility| facility.as_str() == "fabric.host.user-systemd-supervision"
                    ));
                }
                other => panic!("unexpected host compatibility source: {other:?}"),
            }
            assert!(
                requirement
                    .required_facilities()
                    .iter()
                    .any(|facility| facility.as_str() == "fabric.host.user-systemd-supervision")
            );
        }
        _ => panic!("unexpected incompatible host result"),
    }
}

#[test]
fn systemd_process_adapter_requires_explicit_linux_systemd_host_for_construction() {
    let (_tempdir, resolver, config) = process_fixture();
    SystemdProcessAdapter::for_host(&compatible_process_host(), config, resolver).expect("adapter");
}

#[test]
fn systemd_process_adapter_starts_with_public_environment_and_preserves_process_identity() {
    let (_tempdir, resolver, config) = process_fixture();
    let supervisor_state = Arc::new(Mutex::new(SupervisorState::default()));
    let supervisor = Arc::new(TestSupervisor {
        state: Arc::clone(&supervisor_state),
    });
    let mut adapter = SystemdProcessAdapter::with_supervisor(config, resolver, supervisor)
        .expect("process adapter");
    let spec = process_spec("notes.process");
    adapter.prepare(&spec).expect("prepare");
    let instance_id = ProcessInstanceId::new("proc-zero").expect("instance id");
    let mut environment = PreparedProcessEnvironment::default();
    environment
        .insert_public("FABRIC_TEST_VALUE", "hello-process")
        .expect("environment");
    let request = ProcessExecutionRequest {
        instance_id: instance_id.clone(),
        start: StartProcessRequest {
            prepared_process: PreparedProcess::new(spec.clone()),
            process: spec,
            environment,
        },
    };

    let mut execution = adapter.start(request).expect("start");
    let recorded = supervisor_state.lock().expect("supervisor state");
    assert_eq!(recorded.start_requests.len(), 1);
    assert_eq!(
        recorded.start_requests[0].unit_name,
        systemd_unit_name(&instance_id)
    );
    assert!(
        recorded.start_requests[0]
            .executable
            .ends_with(Path::new("artifact").join("run.sh"))
    );
    assert!(
        recorded.start_requests[0]
            .working_directory
            .ends_with(Path::new("artifact"))
    );
    assert_eq!(
        recorded.start_requests[0]
            .environment
            .get("FABRIC_TEST_VALUE"),
        Some(&"hello-process".to_owned())
    );
    drop(recorded);

    execution.stop().expect("stop");
    execution.cleanup();
    let recorded = supervisor_state.lock().expect("supervisor state");
    assert_eq!(
        recorded.stop_requests,
        vec![systemd_unit_name(&instance_id)]
    );
}

#[test]
fn systemd_process_adapter_releases_projection_leases_on_stop_and_failed_start() {
    let (_tempdir, resolver, config) = process_fixture();
    let success_state = Arc::new(Mutex::new(SupervisorState::default()));
    let success_supervisor = Arc::new(TestSupervisor {
        state: Arc::clone(&success_state),
    });
    let mut success_adapter = SystemdProcessAdapter::with_supervisor(
        config.clone(),
        Arc::clone(&resolver),
        success_supervisor,
    )
    .expect("process adapter");
    let spec = process_spec("lease.process");
    success_adapter.prepare(&spec).expect("prepare");
    let released = Arc::new(Mutex::new(false));
    let mut environment = PreparedProcessEnvironment::default();
    let mut leases = ProjectionLeases::default();
    leases.push(TestLease(Arc::clone(&released)));
    environment.append_leases(leases);
    let mut execution = success_adapter
        .start(ProcessExecutionRequest {
            instance_id: ProcessInstanceId::new("proc-lease").expect("process instance"),
            start: StartProcessRequest {
                prepared_process: PreparedProcess::new(spec.clone()),
                process: spec.clone(),
                environment,
            },
        })
        .expect("start");
    assert!(!*released.lock().expect("released"));
    execution.stop().expect("stop");
    execution.cleanup();
    assert!(*released.lock().expect("released"));

    let failure_state = Arc::new(Mutex::new(SupervisorState {
        fail_start: Some("synthetic supervisor failure".to_owned()),
        ..SupervisorState::default()
    }));
    let failure_supervisor = Arc::new(TestSupervisor {
        state: Arc::clone(&failure_state),
    });
    let mut failing_adapter =
        SystemdProcessAdapter::with_supervisor(config, resolver, failure_supervisor)
            .expect("process adapter");
    failing_adapter.prepare(&spec).expect("prepare");
    let failed_released = Arc::new(Mutex::new(false));
    let mut failed_environment = PreparedProcessEnvironment::default();
    let mut failed_leases = ProjectionLeases::default();
    failed_leases.push(TestLease(Arc::clone(&failed_released)));
    failed_environment.append_leases(failed_leases);
    let error = match failing_adapter.start(ProcessExecutionRequest {
        instance_id: ProcessInstanceId::new("proc-fail").expect("process instance"),
        start: StartProcessRequest {
            prepared_process: PreparedProcess::new(spec.clone()),
            process: spec,
            environment: failed_environment,
        },
    }) {
        Ok(_) => panic!("start should fail"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        ProcessError::StartFailed { ref message }
            if message.contains("synthetic supervisor failure")
    ));
    assert!(*failed_released.lock().expect("released"));
}

#[test]
fn systemd_process_adapter_rejects_entrypoint_escape() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let artifact_file = tempdir.path().join("artifact.bin");
    fs::write(&artifact_file, b"artifact").expect("artifact file");
    let safe_root = tempdir.path().join("safe");
    fs::create_dir_all(&safe_root).expect("safe root");
    let nested_root = tempdir.path().join("safe").join("nested");
    fs::create_dir_all(&nested_root).expect("nested root");
    let resolver: Arc<dyn ProcessArtifactResolver> = Arc::new(FixtureResolver {
        artifact_file: artifact_file.clone(),
        execution_root: nested_root,
    });
    let config = SystemdProcessConfig::new(tempdir.path().join("runtime"));
    let supervisor = Arc::new(TestSupervisor {
        state: Arc::new(Mutex::new(SupervisorState::default())),
    });
    let mut adapter = SystemdProcessAdapter::with_supervisor(config, resolver, supervisor)
        .expect("process adapter");
    let error = adapter
        .prepare(&ProcessSpec {
            process_id: ProcessId::new("notes.process").expect("process"),
            artifact: artifact_from_file(&artifact_file),
            entrypoint: ProcessEntrypoint::new("../escape.sh").expect("entrypoint"),
        })
        .expect_err("escaped entrypoint should fail");
    assert!(matches!(
        error,
        ProcessError::PrepareFailed { ref message }
            if message.contains("outside the trusted execution root")
                || message.contains("canonicalize process entrypoint")
    ));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a usable user systemd manager"]
fn real_systemd_process_start_stop() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let (resolver, config, spec) = linux_process_fixture(
        tempdir.path(),
        "#!/bin/sh\ntrap 'exit 0' TERM INT\nwhile true; do sleep 1; done\n",
    );
    let mut adapter = SystemdProcessAdapter::for_host(&compatible_process_host(), config, resolver)
        .expect("adapter");
    adapter.prepare(&spec).expect("prepare");
    let mut execution = adapter
        .start(ProcessExecutionRequest {
            instance_id: ProcessInstanceId::new("proc-real").expect("process instance"),
            start: StartProcessRequest {
                prepared_process: PreparedProcess::new(spec.clone()),
                process: spec,
                environment: PreparedProcessEnvironment::default(),
            },
        })
        .expect("start");
    execution.stop().expect("stop");
    execution.cleanup();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a usable user systemd manager"]
fn real_systemd_process_receives_public_environment() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let output = tempdir.path().join("proof.txt");
    let script = format!(
        "#!/bin/sh\nprintf '%s' \"$FABRIC_TEST_VALUE\" > \"{}\"\ntrap 'exit 0' TERM INT\nwhile true; do sleep 1; done\n",
        output.display()
    );
    let (resolver, config, spec) = linux_process_fixture(tempdir.path(), &script);
    let mut adapter = SystemdProcessAdapter::for_host(&compatible_process_host(), config, resolver)
        .expect("adapter");
    adapter.prepare(&spec).expect("prepare");
    let mut environment = PreparedProcessEnvironment::default();
    environment
        .insert_public("FABRIC_TEST_VALUE", "hello-process")
        .expect("environment");
    let mut execution = adapter
        .start(ProcessExecutionRequest {
            instance_id: ProcessInstanceId::new("proc-env").expect("process instance"),
            start: StartProcessRequest {
                prepared_process: PreparedProcess::new(spec.clone()),
                process: spec,
                environment,
            },
        })
        .expect("start");
    wait_for_file_contents(&output, "hello-process");
    execution.stop().expect("stop");
    execution.cleanup();
}

fn process_fixture() -> (
    tempfile::TempDir,
    Arc<dyn ProcessArtifactResolver>,
    SystemdProcessConfig,
) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let execution_root = tempdir.path().join("artifact-root");
    fs::create_dir_all(&execution_root).expect("execution root");
    let artifact_file = tempdir.path().join("artifact.bin");
    fs::write(&artifact_file, b"process-artifact").expect("artifact file");
    let executable = execution_root.join("run.sh");
    write_executable(
        &executable,
        "#!/bin/sh\nprintf 'process-ok' > ./process-proof.txt\ntrap 'exit 0' TERM INT\nwhile true; do sleep 1; done\n",
    );
    let resolver: Arc<dyn ProcessArtifactResolver> = Arc::new(FixtureResolver {
        artifact_file,
        execution_root,
    });
    let config = SystemdProcessConfig::new(tempdir.path().join("runtime"));
    (tempdir, resolver, config)
}

#[cfg(target_os = "linux")]
fn linux_process_fixture(
    root: &Path,
    script: &str,
) -> (
    Arc<dyn ProcessArtifactResolver>,
    SystemdProcessConfig,
    ProcessSpec,
) {
    let execution_root = root.join("artifact-root");
    fs::create_dir_all(&execution_root).expect("execution root");
    let artifact_file = root.join("artifact.bin");
    fs::write(&artifact_file, b"process-artifact").expect("artifact file");
    let executable = execution_root.join("run.sh");
    write_executable(&executable, script);
    (
        Arc::new(FixtureResolver {
            artifact_file: artifact_file.clone(),
            execution_root,
        }),
        SystemdProcessConfig::new(root.join("runtime")),
        ProcessSpec {
            process_id: ProcessId::new("apps.process").expect("process"),
            artifact: artifact_from_file(&artifact_file),
            entrypoint: ProcessEntrypoint::new("run.sh").expect("entrypoint"),
        },
    )
}

fn process_spec(process_id: &str) -> ProcessSpec {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let artifact_file = tempdir.path().join("artifact.bin");
    fs::write(&artifact_file, b"process-artifact").expect("artifact file");
    ProcessSpec {
        process_id: ProcessId::new(process_id).expect("process id"),
        artifact: artifact_from_file(&artifact_file),
        entrypoint: ProcessEntrypoint::new("run.sh").expect("entrypoint"),
    }
}

fn compatible_process_host() -> HostDescriptor {
    HostDescriptor::new(
        HostOperatingSystem::new("linux").expect("os"),
        HostArchitecture::new("x86_64").expect("arch"),
    )
    .with_facility(
        HostFacilityId::new("fabric.host.user-systemd-supervision").expect("host facility"),
    )
}

fn artifact_from_file(path: &Path) -> ProcessArtifact {
    let bytes = fs::read(path).expect("artifact bytes");
    ProcessArtifact {
        reference: format!("fabric://artifact/{}", path.display()),
        sha256: Sha256::digest(bytes).into(),
    }
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("permissions");
    }
}

#[cfg(target_os = "linux")]
fn wait_for_file_contents(path: &Path, expected: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(contents) = fs::read_to_string(path) {
            if contents == expected {
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!(
        "timed out waiting for file {} to contain {expected}",
        path.display(),
    );
}
