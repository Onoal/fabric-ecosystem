use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fabric::prelude::*;
use fabric_package_process_runtime::{
    local_process_runtime, ProcessEnvironmentVariable, ProcessExecutionError,
    ProcessExecutionErrorKind, ProcessInvocation, ProcessOutput, ProcessRuntime,
};
use futures::executor::block_on;

fabric::component! {
    TestProcessApp {
        id: "onoal.package.execution.process-runtime.test-app";

        relations {
            requires {
                runtime: ProcessRuntime;
            }
        }

        api {
            fn execute(&self, invocation: ProcessInvocation) -> Result<ProcessOutput, ProcessExecutionError>;
        }

        runtime {
            fn execute(&self, invocation: ProcessInvocation) -> Result<ProcessOutput, ProcessExecutionError> {
                resolve_resource(self.relations().runtime.execute(invocation))
            }
        }
    }
}

fabric::component! {
    TestDualProcessApp {
        id: "onoal.package.execution.process-runtime.test-dual-app";

        relations {
            requires {
                primary: ProcessRuntime;
                secondary: ProcessRuntime;
            }
        }

        api {
            fn execute_both(&self, primary: ProcessInvocation, secondary: ProcessInvocation) -> Result<(ProcessOutput, ProcessOutput), ProcessExecutionError>;
        }

        runtime {
            fn execute_both(&self, primary: ProcessInvocation, secondary: ProcessInvocation) -> Result<(ProcessOutput, ProcessOutput), ProcessExecutionError> {
                Ok((
                    resolve_resource(self.relations().primary.execute(primary))?,
                    resolve_resource(self.relations().secondary.execute(secondary))?,
                ))
            }
        }
    }
}

fn bind_process_app(runtime_name: &'static str) -> impl IntoFabricContribution {
    let runtime = ProcessRuntime::select(runtime_name).expect("runtime selection");
    let component = TestProcessApp::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("runtime").expect("role"),
            fabric::authoring::Requires::<ProcessRuntime>::provisional(),
        ),
        &runtime,
    );
    FabricContribution::new().component(component)
}

fn bind_dual_process_app(
    primary_name: &'static str,
    secondary_name: &'static str,
) -> impl IntoFabricContribution {
    let primary = ProcessRuntime::select(primary_name).expect("primary selection");
    let secondary = ProcessRuntime::select(secondary_name).expect("secondary selection");
    let component = TestDualProcessApp::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("primary").expect("role"),
                fabric::authoring::Requires::<ProcessRuntime>::provisional(),
            ),
            &primary,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("secondary").expect("role"),
                fabric::authoring::Requires::<ProcessRuntime>::provisional(),
            ),
            &secondary,
        );
    FabricContribution::new().component(component)
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("process runtime test resource operation unexpectedly yielded")
        }
    }
}

fn process_composition(id: &str) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(local_process_runtime("local"))
        .with(bind_process_app("local"))
        .build()
        .expect("composition")
}

fn started_instance(composition: &Composition, id: &str) -> Instance {
    let mut instance = composition
        .materialize_on(id, &HostDescriptor::native())
        .expect("instance");
    instance.start().expect("start");
    instance
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "onoal-fabric-process-runtime-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

#[cfg(unix)]
fn shell(script: &str) -> ProcessInvocation {
    ProcessInvocation::new("sh").with_args(["-c", script])
}

#[cfg(windows)]
fn shell(script: &str) -> ProcessInvocation {
    ProcessInvocation::new("cmd").with_args(["/C", script])
}

#[test]
fn composition_declares_only_process_runtime_resource() {
    let composition = Fabric::new("onoal.package.test.process.inspect")
        .expect("fabric")
        .with(local_process_runtime("local"))
        .build()
        .expect("composition");

    assert_eq!(composition.resources().count(), 1);
    assert_eq!(composition.systems().count(), 0);
    assert_eq!(composition.components().count(), 0);

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
}

#[test]
fn arguments_stdout_stderr_and_non_zero_exit_are_process_outcome() {
    let composition = process_composition("onoal.package.test.process.outcome");
    let mut instance =
        started_instance(&composition, "onoal.package.test.process.outcome.instance");
    let app = instance.component::<TestProcessApp>().expect("app");
    app.reconcile().expect("reconcile");

    #[cfg(unix)]
    let invocation = ProcessInvocation::new("sh").with_args([
        "-c",
        "printf '%s|%s' \"$1\" \"$2\"; printf 'err:%s' \"$3\" >&2; exit 7",
        "ignored-script-name",
        "alpha",
        "beta",
        "gamma",
    ]);
    #[cfg(windows)]
    let invocation = ProcessInvocation::new("cmd")
        .with_args(["/C", "echo alpha|beta&&echo err:gamma 1>&2&&exit /B 7"]);

    let output = block_on(app.execute(invocation))
        .expect("component call")
        .expect("process output");
    assert!(!output.success());
    assert_eq!(output.status_code, Some(7));
    #[cfg(unix)]
    assert_eq!(output.stdout, b"alpha|beta".to_vec());
    #[cfg(windows)]
    assert!(output.stdout_utf8().contains("alpha"));
    assert!(output.stderr_utf8().contains("err:gamma"));

    instance.stop().expect("stop");
}

#[test]
fn spawn_failure_is_execution_error_not_output() {
    let composition = process_composition("onoal.package.test.process.spawn-failure");
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.process.spawn-failure.instance",
    );
    let app = instance.component::<TestProcessApp>().expect("app");
    app.reconcile().expect("reconcile");

    let error = block_on(app.execute(ProcessInvocation::new(
        "onoal-definitely-missing-executable-for-process-runtime-test",
    )))
    .expect("component call")
    .expect_err("spawn failure");
    assert_eq!(error.kind, ProcessExecutionErrorKind::SpawnFailed);

    instance.stop().expect("stop");
}

#[test]
fn working_directory_environment_and_stdin_are_invocation_inputs() {
    let directory = unique_temp_dir("cwd");
    std::fs::write(directory.join("marker.txt"), b"marker").expect("marker file");
    let composition = process_composition("onoal.package.test.process.invocation-inputs");
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.process.invocation-inputs.instance",
    );
    let app = instance.component::<TestProcessApp>().expect("app");
    app.reconcile().expect("reconcile");

    #[cfg(unix)]
    let invocation =
        shell("printf 'cwd='; pwd; printf '\\nenv=%s\\nstdin=' \"$ONOAL_PROCESS_TEST\"; cat")
            .with_working_directory(directory.clone())
            .with_environment([ProcessEnvironmentVariable::new(
                "ONOAL_PROCESS_TEST",
                "visible",
            )])
            .with_stdin(b"from-stdin".to_vec());
    #[cfg(windows)]
    let invocation = shell("cd && echo env=%ONOAL_PROCESS_TEST%")
        .with_working_directory(directory.clone())
        .with_environment([ProcessEnvironmentVariable::new(
            "ONOAL_PROCESS_TEST",
            "visible",
        )])
        .with_stdin(b"from-stdin".to_vec());

    let output = block_on(app.execute(invocation))
        .expect("component call")
        .expect("process output");
    assert!(output.success());
    let stdout = output.stdout_utf8();
    assert!(stdout.contains(directory.to_string_lossy().as_ref()));
    assert!(stdout.contains("env=visible"));
    #[cfg(unix)]
    assert!(stdout.contains("stdin=from-stdin"));

    instance.stop().expect("stop");
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn multiple_named_process_runtimes_can_coexist() {
    let composition = Fabric::new("onoal.package.test.process.multiple")
        .expect("fabric")
        .with(local_process_runtime("primary"))
        .with(local_process_runtime("secondary"))
        .with(bind_dual_process_app("primary", "secondary"))
        .build()
        .expect("composition");
    assert_eq!(composition.resources().count(), 2);

    let mut instance =
        started_instance(&composition, "onoal.package.test.process.multiple.instance");
    let app = instance
        .component::<TestDualProcessApp>()
        .expect("dual app");
    app.reconcile().expect("reconcile");
    let (primary, secondary) =
        block_on(app.execute_both(shell("printf primary"), shell("printf secondary")))
            .expect("component call")
            .expect("process output");
    assert_eq!(primary.stdout_utf8(), "primary");
    assert_eq!(secondary.stdout_utf8(), "secondary");

    instance.stop().expect("stop");
}

#[test]
fn stopped_instance_does_not_fabricate_successful_execution() {
    let composition = process_composition("onoal.package.test.process.lifecycle");
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.process.lifecycle.instance",
    );
    instance.stop().expect("stop");

    if let Ok(app) = instance.component::<TestProcessApp>() {
        let result = block_on(app.execute(shell("printf after-stop")));
        match result {
            Ok(Err(error)) => assert_eq!(error.kind, ProcessExecutionErrorKind::Stopped),
            Err(_) => {}
            Ok(Ok(output)) => panic!("stopped runtime fabricated output: {output:?}"),
        }
    }
}
