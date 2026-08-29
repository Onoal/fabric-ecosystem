#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::process::Command;

use fabric_resource_process::ProcessError;

use crate::adapter::runtime::{ProcessSupervisor, StartUnitRequest};

pub(crate) struct SystemdProcessSupervisor;

impl SystemdProcessSupervisor {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ProcessSupervisor for SystemdProcessSupervisor {
    fn start_transient_unit(&self, request: &StartUnitRequest<'_>) -> Result<(), ProcessError> {
        #[cfg(target_os = "linux")]
        {
            start_transient_unit(request)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = request;
            Err(ProcessError::StartFailed {
                message: "systemd process adapter currently requires Linux systemd".to_owned(),
            })
        }
    }

    fn stop_unit(&self, unit_name: &str) -> Result<(), ProcessError> {
        #[cfg(target_os = "linux")]
        {
            stop_unit(unit_name)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = unit_name;
            Err(ProcessError::StopFailed {
                message: "systemd process adapter currently requires Linux systemd".to_owned(),
            })
        }
    }

    fn active_state(&self, unit_name: &str) -> Result<Option<String>, ProcessError> {
        #[cfg(target_os = "linux")]
        {
            active_state(unit_name)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = unit_name;
            Ok(None)
        }
    }

    fn reset_failed(&self, unit_name: &str) -> Result<(), ProcessError> {
        #[cfg(target_os = "linux")]
        {
            reset_failed(unit_name)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = unit_name;
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn start_transient_unit(request: &StartUnitRequest<'_>) -> Result<(), ProcessError> {
    let mut command = user_systemd_command("systemd-run");
    command
        .arg("--quiet")
        .arg("--collect")
        .arg(format!("--unit={}", request.unit_name))
        .arg("--property=Type=simple")
        .arg("--property=Restart=no")
        .arg("--property=KillMode=control-group")
        .arg(format!(
            "--working-directory={}",
            request.working_directory.display()
        ));
    append_environment(&mut command, request.environment);
    command.arg("--").arg(request.executable);
    run_systemd_command(
        command,
        "start process transient unit",
        ProcessErrorKind::Start,
    )
}

#[cfg(target_os = "linux")]
fn stop_unit(unit_name: &str) -> Result<(), ProcessError> {
    let mut command = user_systemd_command("systemctl");
    command.arg("stop").arg(unit_name);
    match command.output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) if is_missing_unit(&output.stderr) => Ok(()),
        Ok(output) => Err(ProcessError::StopFailed {
            message: format!(
                "stop process transient unit: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }),
        Err(error) => Err(ProcessError::StopFailed {
            message: format!("stop process transient unit: {error}"),
        }),
    }
}

#[cfg(target_os = "linux")]
fn active_state(unit_name: &str) -> Result<Option<String>, ProcessError> {
    let mut command = user_systemd_command("systemctl");
    command
        .arg("show")
        .arg(unit_name)
        .arg("--property=ActiveState")
        .arg("--value");
    match command.output() {
        Ok(output) if output.status.success() => {
            let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if state.is_empty() {
                Ok(None)
            } else {
                Ok(Some(state))
            }
        }
        Ok(output) if is_missing_unit(&output.stderr) => Ok(None),
        Ok(output) => Err(ProcessError::StartFailed {
            message: format!(
                "inspect process transient unit: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }),
        Err(error) => Err(ProcessError::StartFailed {
            message: format!("inspect process transient unit: {error}"),
        }),
    }
}

#[cfg(target_os = "linux")]
fn reset_failed(unit_name: &str) -> Result<(), ProcessError> {
    let mut command = user_systemd_command("systemctl");
    command.arg("reset-failed").arg(unit_name);
    match command.output() {
        Ok(output) if output.status.success() || is_missing_unit(&output.stderr) => Ok(()),
        Ok(output) => Err(ProcessError::StopFailed {
            message: format!(
                "reset process transient unit: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }),
        Err(error) => Err(ProcessError::StopFailed {
            message: format!("reset process transient unit: {error}"),
        }),
    }
}

#[cfg(target_os = "linux")]
fn user_systemd_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.arg("--user");
    command
}

#[cfg(target_os = "linux")]
fn append_environment(command: &mut Command, environment: &BTreeMap<String, String>) {
    for (name, value) in environment {
        command.arg(format!("--setenv={name}={value}"));
    }
}

#[cfg(target_os = "linux")]
fn run_systemd_command(
    mut command: Command,
    context: &str,
    error_kind: ProcessErrorKind,
) -> Result<(), ProcessError> {
    match command.output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(error_kind.into_error(format!(
            "{context}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
        Err(error) => Err(error_kind.into_error(format!("{context}: {error}"))),
    }
}

#[cfg(target_os = "linux")]
fn is_missing_unit(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr);
    stderr.contains("not loaded") || stderr.contains("not-found")
}

#[cfg(target_os = "linux")]
enum ProcessErrorKind {
    Start,
    Stop,
}

#[cfg(target_os = "linux")]
impl ProcessErrorKind {
    fn into_error(self, message: String) -> ProcessError {
        match self {
            Self::Start => ProcessError::StartFailed { message },
            Self::Stop => ProcessError::StopFailed { message },
        }
    }
}
