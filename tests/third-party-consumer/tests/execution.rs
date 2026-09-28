mod support;

use fabric::prelude::*;
use fabric_package_process_runtime::{
    ProcessEnvironmentVariable, ProcessExecutionError, ProcessInvocation, ProcessOutput,
    ProcessRuntime,
};
use futures::executor::block_on;
use support::fabric::{activate, started_instance};

fabric::component! {
    ProcessConsumer {
        id: "onoal.test.third-party.execution.process-consumer";

        relations {
            requires {
                runtime: ProcessRuntime;
            }
        }

        api {
            fn run_process(&self) -> Result<ProcessOutput, ProcessExecutionError>;
        }

        runtime {
            fn run_process(&self) -> Result<ProcessOutput, ProcessExecutionError> {
                resolve_resource(self.relations().runtime.execute(
                    ProcessInvocation::new("sh")
                        .with_args([
                            "-c",
                            "printf '%s:%s' \"$1\" \"$ONOAL_THIRD_PARTY_PROCESS\"",
                            "ignored-script-name",
                            "third-party",
                        ])
                        .with_environment([ProcessEnvironmentVariable::new(
                            "ONOAL_THIRD_PARTY_PROCESS",
                            "process",
                        )]),
                ))
            }
        }
    }
}

fn process_consumer(runtime_name: &'static str) -> impl IntoFabricContribution {
    let runtime = ProcessRuntime::select(runtime_name).expect("process selection");
    let component = ProcessConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("runtime").expect("role"),
            fabric::authoring::Requires::<ProcessRuntime>::provisional(),
        ),
        &runtime,
    );
    FabricContribution::new().component(component)
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("third-party process resource operation unexpectedly yielded")
        }
    }
}

#[cfg(unix)]
#[test]
fn external_consumer_executes_process_invocation_to_completion() {
    let composition = Fabric::new("onoal.test.third-party.execution")
        .expect("fabric")
        .with(fabric_package_process_runtime::local_process_runtime(
            "local",
        ))
        .with(process_consumer("local"))
        .build()
        .expect("composition");
    let instance = started_instance(&composition, "onoal.test.third-party.execution.instance");
    let consumer = activate::<ProcessConsumer>(&instance);

    let output = block_on(consumer.run_process())
        .expect("exercise")
        .expect("process output");

    assert!(output.success());
    assert_eq!(output.stdout_utf8(), "third-party:process");
    assert_eq!(output.stderr, Vec::<u8>::new());
}
