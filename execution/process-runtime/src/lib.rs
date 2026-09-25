//! Local process execution package for Fabric.
//!
//! This is a bounded execution capability, not a supervisor, deployment
//! controller, or restart manager.

use std::process::Command;

use fabric::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionEnvironment {
    pub os: String,
    pub arch: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub spawn_error: Option<String>,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.status_code == Some(0) && self.spawn_error.is_none()
    }

    pub fn stdout_utf8(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessProbeInput {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessProbeOutput {
    pub success: bool,
    pub stdout: String,
    pub environment: ExecutionEnvironment,
}

fabric::system! {
    pub LocalExecutionEnvironment {
        id: "onoal.package.execution.local-environment";

        api {
            fn describe(&self) -> ExecutionEnvironment;
        }

        runtime {
            fn describe(&self) -> ExecutionEnvironment {
                ExecutionEnvironment {
                    os: std::env::consts::OS.to_owned(),
                    arch: std::env::consts::ARCH.to_owned(),
                }
            }
        }
    }
}

fabric::resource! {
    pub ProcessRuntime {
        id: "onoal.package.execution.process-runtime";

        api {
            fn run(&self, command: String, args: Vec<String>) -> ProcessOutput;
        }
    }
}

fabric::adapter! {
    pub LocalProcessRuntime for ProcessRuntime {
        id: "onoal.package.execution.process-runtime.local";

        runtime {
            fn run(&self, command: String, args: Vec<String>) -> ProcessOutput {
                match Command::new(command).args(args).output() {
                    Ok(output) => ProcessOutput {
                        status_code: output.status.code(),
                        stdout: output.stdout,
                        stderr: output.stderr,
                        spawn_error: None,
                    },
                    Err(error) => ProcessOutput {
                        status_code: None,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        spawn_error: Some(error.to_string()),
                    },
                }
            }
        }
    }
}

fabric::component! {
    pub ProcessProbe {
        id: "onoal.package.execution.process-probe";

        relations {
            requires {
                runtime: ProcessRuntime;
                environment: LocalExecutionEnvironment;
            }
        }

        api {
            fn probe(&self, input: ProcessProbeInput) -> ProcessProbeOutput;
        }

        runtime {
            fn probe(&self, input: ProcessProbeInput) -> ProcessProbeOutput {
                let output = self.relations().runtime.run(input.command, input.args);
                ProcessProbeOutput {
                    success: output.success(),
                    stdout: output.stdout_utf8(),
                    environment: self.relations().environment.describe(),
                }
            }
        }
    }
}

pub fn local_execution_environment() -> impl IntoFabricContribution {
    FabricContribution::new()
        .system(LocalExecutionEnvironment::select().expect("local execution environment"))
}

pub fn local_process_runtime(name: &'static str) -> impl IntoFabricContribution {
    let runtime = ProcessRuntime::select(name).expect("valid ProcessRuntime resource name");
    FabricContribution::new().resource(
        runtime
            .using(LocalProcessRuntime::new())
            .expect("LocalProcessRuntime supports ProcessRuntime"),
    )
}

pub fn process_probe(runtime_name: &'static str) -> impl IntoFabricContribution {
    let runtime = ProcessRuntime::select(runtime_name).expect("valid ProcessRuntime resource name");
    let component = ProcessProbe::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("runtime").expect("role"),
            fabric::authoring::Requires::<ProcessRuntime>::provisional(),
        ),
        &runtime,
    );

    FabricContribution::new().component(component)
}

pub fn local_process_platform(runtime_name: &'static str) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(local_execution_environment())
        .with(local_process_runtime(runtime_name))
        .with(process_probe(runtime_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn process_package_exposes_resource_system_component_truth() {
        let composition = Fabric::new("onoal.package.test.process.inspect")
            .expect("fabric")
            .with(local_process_platform("local"))
            .build()
            .expect("composition");

        assert_eq!(composition.resources().count(), 1);
        assert_eq!(composition.systems().count(), 1);
        assert_eq!(composition.components().count(), 1);

        let runtime = composition.resources().next().expect("runtime");
        assert_eq!(
            runtime.resource_id().as_str(),
            "onoal.package.execution.process-runtime"
        );
        assert_eq!(
            runtime
                .realization()
                .adapter_definition_id()
                .expect("adapter")
                .as_str(),
            "onoal.package.execution.process-runtime.local"
        );

        let component = composition.components().next().expect("component");
        let roles = component
            .relations()
            .map(|relation| relation.role().as_str())
            .collect::<Vec<_>>();
        assert!(roles.contains(&"runtime"));
        assert!(roles.contains(&"environment"));
        assert!(composition.relations().iter().any(|relation| {
            relation.role().as_str() == "runtime"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { .. }
                )
        }));
    }

    #[test]
    fn local_process_runtime_runs_one_os_process() {
        let composition = Fabric::new("onoal.package.test.process.run")
            .expect("fabric")
            .with(local_process_platform("local"))
            .build()
            .expect("composition");
        let mut instance = composition
            .materialize_on(
                "onoal.package.test.process.run.instance",
                &HostDescriptor::native(),
            )
            .expect("instance");
        instance.start().expect("start");

        let probe = instance.component::<ProcessProbe>().expect("probe");
        probe.reconcile().expect("component reconcile");
        let output = block_on(probe.probe(ProcessProbeInput {
            command: "sh".to_owned(),
            args: vec!["-c".to_owned(), "printf package-process".to_owned()],
        }))
        .expect("run process");

        assert!(output.success);
        assert_eq!(output.stdout, "package-process");
    }

    #[test]
    fn process_probe_invokes_through_component_participation() {
        let composition = Fabric::new("onoal.package.test.process.component")
            .expect("fabric")
            .with(local_process_platform("local"))
            .build()
            .expect("composition");
        let mut instance = composition
            .materialize_on(
                "onoal.package.test.process.component.instance",
                &HostDescriptor::native(),
            )
            .expect("instance");
        instance.start().expect("start");

        let probe = instance.component::<ProcessProbe>().expect("probe");
        probe.reconcile().expect("component reconcile");
        let output = block_on(probe.probe(ProcessProbeInput {
            command: "sh".to_owned(),
            args: vec!["-c".to_owned(), "printf component-process".to_owned()],
        }))
        .expect("probe invoke");

        assert!(output.success);
        assert_eq!(output.stdout, "component-process");
        assert!(!output.environment.os.is_empty());
    }
}
