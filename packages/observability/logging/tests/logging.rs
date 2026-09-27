use fabric::prelude::*;
use fabric_package_observability_logging::{
    console_logging, ConsoleLogConfig, ConsoleLogSink, ConsoleLogSinkConfig, LogError, LogLevel,
    LogRecord, LogSink,
};
use futures::executor::block_on;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestEmissionResult {
    emitted: Vec<LogRecord>,
}

fabric::component! {
    TestLoggingApp {
        id: "onoal.package.observability.logging.test-app";

        relations {
            requires {
                sink: LogSink;
            }
        }

        api {
            fn emit_info(&self, message: String) -> Result<(), LogError>;
            fn emit_records(&self) -> Result<TestEmissionResult, LogError>;
        }

        runtime {
            fn emit_info(&self, message: String) -> Result<(), LogError> {
                self.relations().sink.emit(LogRecord::new(LogLevel::Info, message))
            }

            fn emit_records(&self) -> Result<TestEmissionResult, LogError> {
                let records = vec![
                    LogRecord::targeted(LogLevel::Info, "application", "application started"),
                    LogRecord::targeted(LogLevel::Warn, "application", "application warning"),
                    LogRecord::targeted(LogLevel::Error, "application", "application error"),
                ];
                for record in records.clone() {
                    self.relations().sink.emit(record)?;
                }
                Ok(TestEmissionResult { emitted: records })
            }
        }
    }
}

fabric::component! {
    TestDualLoggingApp {
        id: "onoal.package.observability.logging.test-dual-app";

        relations {
            requires {
                application: LogSink;
                security: LogSink;
            }
        }

        api {
            fn emit_to_both(&self) -> Result<TestEmissionResult, LogError>;
        }

        runtime {
            fn emit_to_both(&self) -> Result<TestEmissionResult, LogError> {
                let application = LogRecord::targeted(
                    LogLevel::Info,
                    "application",
                    "application event",
                );
                let security = LogRecord::targeted(
                    LogLevel::Warn,
                    "security",
                    "security event",
                );
                self.relations().application.emit(application.clone())?;
                self.relations().security.emit(security.clone())?;
                Ok(TestEmissionResult {
                    emitted: vec![application, security],
                })
            }
        }
    }
}

fn started_instance(composition: &Composition, id: &str) -> Instance {
    let mut instance = composition
        .materialize_on(id, &HostDescriptor::native())
        .expect("instance");
    instance.start().expect("start");
    instance
}

fn activate<C: fabric::authoring::ComponentDefinition>(
    instance: &Instance,
) -> BoundComponent<'_, C> {
    let component = instance.component::<C>().expect("component");
    component.reconcile().expect("reconcile");
    component
}

fn logging_app(sink_name: &'static str) -> impl IntoFabricContribution {
    let sink = LogSink::select(sink_name).expect("sink selection");
    let component = TestLoggingApp::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("sink").expect("role"),
            fabric::authoring::Requires::<LogSink>::provisional(),
        ),
        &sink,
    );
    FabricContribution::new().component(component)
}

fn dual_logging_app(
    application_name: &'static str,
    security_name: &'static str,
) -> impl IntoFabricContribution {
    let application = LogSink::select(application_name).expect("application sink");
    let security = LogSink::select(security_name).expect("security sink");
    let component = TestDualLoggingApp::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("application").expect("role"),
                fabric::authoring::Requires::<LogSink>::provisional(),
            ),
            &application,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("security").expect("role"),
                fabric::authoring::Requires::<LogSink>::provisional(),
            ),
            &security,
        );
    FabricContribution::new().component(component)
}

#[test]
fn log_record_model_and_level_ordering_are_bounded() {
    assert!(LogLevel::Trace < LogLevel::Debug);
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warn);
    assert!(LogLevel::Warn < LogLevel::Error);

    assert_eq!(LogLevel::Error.to_string(), "ERROR");
    assert_eq!(
        LogRecord::new(LogLevel::Info, "hello"),
        LogRecord {
            level: LogLevel::Info,
            message: "hello".to_owned(),
            target: None,
        }
    );
    assert_eq!(
        LogRecord::targeted(LogLevel::Warn, "application", "slow").target,
        Some("application".to_owned())
    );
}

#[test]
fn composition_declares_log_sink_without_global_logger() {
    let helper = Fabric::new("onoal.package.test.logging.inspect.helper")
        .expect("fabric")
        .with(console_logging("application"))
        .build()
        .expect("composition");
    let explicit = Fabric::new("onoal.package.test.logging.inspect.explicit")
        .expect("fabric")
        .resource(
            LogSink::select("application")
                .expect("log sink")
                .using(ConsoleLogSink::new(ConsoleLogSinkConfig {
                    config: ConsoleLogConfig::default(),
                }))
                .expect("adapter"),
        )
        .build()
        .expect("composition");

    let helper_sink = helper.resources().next().expect("helper sink");
    let explicit_sink = explicit.resources().next().expect("explicit sink");
    assert_eq!(helper_sink.resource_id(), explicit_sink.resource_id());
    assert_eq!(helper_sink.name(), explicit_sink.name());
    assert_eq!(
        helper_sink
            .realization()
            .adapter_definition_id()
            .expect("helper adapter"),
        explicit_sink
            .realization()
            .adapter_definition_id()
            .expect("explicit adapter")
    );
    assert_eq!(helper.components().count(), 0);
    assert_eq!(helper.resources().count(), 1);
}

#[test]
fn consumer_owned_component_emits_records_through_log_sink() {
    let composition = Fabric::new("onoal.package.test.logging.emit")
        .expect("fabric")
        .with(console_logging("application"))
        .with(logging_app("application"))
        .build()
        .expect("composition");
    let instance = started_instance(&composition, "onoal.package.test.logging.emit.instance");
    let app = activate::<TestLoggingApp>(&instance);

    let result = block_on(app.emit_records())
        .expect("emit call")
        .expect("emit ok");
    assert_eq!(
        result
            .emitted
            .iter()
            .map(|record| record.level)
            .collect::<Vec<_>>(),
        vec![LogLevel::Info, LogLevel::Warn, LogLevel::Error]
    );
}

#[test]
fn multiple_named_log_sinks_are_bound_explicitly_without_singleton_state() {
    let composition = Fabric::new("onoal.package.test.logging.occurrences")
        .expect("fabric")
        .with(console_logging("application"))
        .with(console_logging("security"))
        .with(dual_logging_app("application", "security"))
        .build()
        .expect("composition");

    assert_eq!(composition.resources().count(), 2);
    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "application"));
    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "security"));
    assert!(composition
        .relations()
        .iter()
        .any(|relation| relation.role().as_str() == "application"));
    assert!(composition
        .relations()
        .iter()
        .any(|relation| relation.role().as_str() == "security"));

    let instance = started_instance(
        &composition,
        "onoal.package.test.logging.occurrences.instance",
    );
    let app = activate::<TestDualLoggingApp>(&instance);
    let result = block_on(app.emit_to_both())
        .expect("dual emit call")
        .expect("dual emit ok");
    assert_eq!(result.emitted.len(), 2);
    assert_eq!(result.emitted[0].target.as_deref(), Some("application"));
    assert_eq!(result.emitted[1].target.as_deref(), Some("security"));
}

#[test]
fn stopped_instance_rejects_logging_before_log_error_is_reached() {
    let composition = Fabric::new("onoal.package.test.logging.lifecycle")
        .expect("fabric")
        .with(console_logging("application"))
        .with(logging_app("application"))
        .build()
        .expect("composition");
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.logging.lifecycle.instance",
    );
    {
        let app = activate::<TestLoggingApp>(&instance);
        block_on(app.emit_info("before stop".to_owned()))
            .expect("emit call")
            .expect("emit ok");
    }
    instance.stop().expect("stop");
    let stopped = instance.component::<TestLoggingApp>().expect("stopped app");
    assert!(block_on(stopped.emit_info("after stop".to_owned())).is_err());
}
