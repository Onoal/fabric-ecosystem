use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{ProcessExecutionError, ProcessInvocation, ProcessOutput, ProcessRuntime};

#[derive(Default)]
struct LocalProcessRuntimeState {
    live: AtomicBool,
    stopped: AtomicBool,
}

impl LocalProcessRuntimeState {
    fn ensure_live(&self) -> Result<(), ProcessExecutionError> {
        if self.live.load(Ordering::SeqCst) {
            Ok(())
        } else if self.stopped.load(Ordering::SeqCst) {
            Err(ProcessExecutionError::stopped())
        } else {
            Err(ProcessExecutionError::not_started())
        }
    }
}

fabric::adapter! {
    pub LocalProcessRuntime for ProcessRuntime {
        id: "onoal.package.execution.process-runtime.local";

        state {
            LocalProcessRuntimeState = LocalProcessRuntimeState::default();
        }

        runtime {
            fn execute(
                &self,
                invocation: ProcessInvocation,
            ) -> Result<ProcessOutput, ProcessExecutionError> {
                self.state.get().ensure_live()?;
                execute_local(invocation)
            }
        }

        lifecycle {
            start {
                self.state.get().stopped.store(false, Ordering::SeqCst);
                self.state.get().live.store(true, Ordering::SeqCst);
                Ok(())
            }

            stop {
                self.state.get().live.store(false, Ordering::SeqCst);
                self.state.get().stopped.store(true, Ordering::SeqCst);
                Ok(())
            }
        }
    }
}

fn execute_local(invocation: ProcessInvocation) -> Result<ProcessOutput, ProcessExecutionError> {
    let mut command = Command::new(&invocation.program);
    command.args(&invocation.args);
    if let Some(working_directory) = invocation.working_directory {
        command.current_dir(working_directory);
    }
    for variable in invocation.environment {
        command.env(variable.name, variable.value);
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    match invocation.stdin {
        Some(stdin) => {
            command.stdin(Stdio::piped());
            let mut child = command
                .spawn()
                .map_err(|error| ProcessExecutionError::spawn_failed(error.to_string()))?;
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(&stdin).map_err(|error| {
                    ProcessExecutionError::stdin_write_failed(error.to_string())
                })?;
            }
            child
                .wait_with_output()
                .map(process_output)
                .map_err(|error| ProcessExecutionError::wait_failed(error.to_string()))
        }
        None => {
            command.stdin(Stdio::null());
            command
                .output()
                .map(process_output)
                .map_err(|error| ProcessExecutionError::spawn_failed(error.to_string()))
        }
    }
}

fn process_output(output: std::process::Output) -> ProcessOutput {
    ProcessOutput {
        status_code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}
