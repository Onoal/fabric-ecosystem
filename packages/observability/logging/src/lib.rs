//! Semantic logging package for Fabric.
//!
//! Logging is application/system behavior that explicitly emits log records.
//! It is not Fabric Instance observation, lifecycle diagnostics, tracing,
//! audit evidence, history, identity, or durable proof.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use fabric::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trace => f.write_str("TRACE"),
            Self::Debug => f.write_str("DEBUG"),
            Self::Info => f.write_str("INFO"),
            Self::Warn => f.write_str("WARN"),
            Self::Error => f.write_str("ERROR"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRecord {
    pub level: LogLevel,
    pub message: String,
    pub target: Option<String>,
}

impl LogRecord {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            target: None,
        }
    }

    pub fn targeted(
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            level,
            message: message.into(),
            target: Some(target.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogErrorKind {
    NotStarted,
    Stopped,
    WriteFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogError {
    pub kind: LogErrorKind,
    pub detail: String,
}

impl LogError {
    pub fn new(kind: LogErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn not_started() -> Self {
        Self::new(LogErrorKind::NotStarted, "log sink has not started")
    }

    pub fn stopped() -> Self {
        Self::new(LogErrorKind::Stopped, "log sink generation is stopped")
    }
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for LogError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLogConfig {
    pub minimum_level: LogLevel,
    pub stderr_from: LogLevel,
    pub prefix: Option<String>,
}

impl Default for ConsoleLogConfig {
    fn default() -> Self {
        Self {
            minimum_level: LogLevel::Trace,
            stderr_from: LogLevel::Warn,
            prefix: None,
        }
    }
}

#[derive(Default)]
pub struct ConsoleLogSinkState {
    live: AtomicBool,
    stopped: AtomicBool,
}

impl ConsoleLogSinkState {
    fn ensure_live(&self) -> Result<(), LogError> {
        if self.live.load(Ordering::SeqCst) {
            Ok(())
        } else if self.stopped.load(Ordering::SeqCst) {
            Err(LogError::stopped())
        } else {
            Err(LogError::not_started())
        }
    }
}

fabric::resource! {
    pub LogSink {
        id: "onoal.package.observability.logging.sink";

        api {
            fn emit(&self, record: LogRecord) -> Result<(), LogError>;
        }
    }
}

fabric::adapter! {
    pub ConsoleLogSink for LogSink {
        id: "onoal.package.observability.logging.console";

        config {
            config: ConsoleLogConfig;
        }

        state {
            ConsoleLogSinkState = ConsoleLogSinkState::default();
        }

        runtime {
            fn emit(&self, record: LogRecord) -> Result<(), LogError> {
                self.state.get().ensure_live()?;
                if record.level < self.config.config.minimum_level {
                    return Ok(());
                }
                let line = format_console_record(&self.config.config, &record);
                if record.level >= self.config.config.stderr_from {
                    eprintln!("{line}");
                } else {
                    println!("{line}");
                }
                Ok(())
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

fn format_console_record(config: &ConsoleLogConfig, record: &LogRecord) -> String {
    let target = record
        .target
        .as_ref()
        .map(|target| format!(" {target}:"))
        .unwrap_or_default();
    match &config.prefix {
        Some(prefix) => format!("{prefix} [{}]{target} {}", record.level, record.message),
        None => format!("[{}]{target} {}", record.level, record.message),
    }
}

pub fn console_logging(name: &'static str) -> impl IntoFabricContribution {
    console_logging_with(name, ConsoleLogConfig::default())
}

pub fn console_logging_with(
    name: &'static str,
    config: ConsoleLogConfig,
) -> impl IntoFabricContribution {
    let selected = LogSink::select(name).expect("valid LogSink resource name");
    FabricContribution::new().resource(
        selected
            .using(ConsoleLogSink::new(ConsoleLogSinkConfig { config }))
            .expect("ConsoleLogSink supports LogSink"),
    )
}

pub fn log_sink_requirement() -> fabric::authoring::Requires<LogSink> {
    fabric::authoring::Requires::<LogSink>::provisional()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEmissionResult {
    pub emitted: Vec<LogRecord>,
}

fabric::component! {
    pub LogEmitter {
        id: "onoal.package.observability.logging.emitter";

        relations {
            requires {
                sink: LogSink;
            }
        }

        api {
            fn emit_info(&self, message: String) -> Result<(), LogError>;
            fn emit_standard_records(&self) -> Result<LogEmissionResult, LogError>;
        }

        runtime {
            fn emit_info(&self, message: String) -> Result<(), LogError> {
                self.relations()
                    .sink
                    .emit(LogRecord::new(LogLevel::Info, message))
            }

            fn emit_standard_records(&self) -> Result<LogEmissionResult, LogError> {
                let records = vec![
                    LogRecord::targeted(LogLevel::Info, "application", "application started"),
                    LogRecord::targeted(LogLevel::Warn, "application", "application warning"),
                    LogRecord::targeted(LogLevel::Error, "application", "application error"),
                ];
                for record in records.clone() {
                    self.relations().sink.emit(record)?;
                }
                Ok(LogEmissionResult { emitted: records })
            }
        }
    }
}

fabric::component! {
    pub DualLogEmitter {
        id: "onoal.package.observability.logging.dual-emitter";

        relations {
            requires {
                application: LogSink;
                security: LogSink;
            }
        }

        api {
            fn emit_to_both(&self) -> Result<LogEmissionResult, LogError>;
        }

        runtime {
            fn emit_to_both(&self) -> Result<LogEmissionResult, LogError> {
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
                Ok(LogEmissionResult {
                    emitted: vec![application, security],
                })
            }
        }
    }
}

pub fn log_emitter(sink_name: &'static str) -> impl IntoFabricContribution {
    let sink = LogSink::select(sink_name).expect("valid LogSink resource name");
    let component = LogEmitter::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("sink").expect("role"),
            fabric::authoring::Requires::<LogSink>::provisional(),
        ),
        &sink,
    );
    FabricContribution::new().component(component)
}

pub fn dual_log_emitter(
    application_name: &'static str,
    security_name: &'static str,
) -> impl IntoFabricContribution {
    let application = LogSink::select(application_name).expect("valid application LogSink name");
    let security = LogSink::select(security_name).expect("valid security LogSink name");
    let component = DualLogEmitter::define()
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

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
    fn component_emits_info_warn_and_error_records_through_log_sink() {
        let composition = Fabric::new("onoal.package.test.logging.emit")
            .expect("fabric")
            .with(console_logging("application"))
            .with(log_emitter("application"))
            .build()
            .expect("composition");
        let instance = started_instance(&composition, "onoal.package.test.logging.emit.instance");
        let emitter = activate::<LogEmitter>(&instance);

        let result = block_on(emitter.emit_standard_records())
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
            .with(dual_log_emitter("application", "security"))
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
        let emitter = activate::<DualLogEmitter>(&instance);
        let result = block_on(emitter.emit_to_both())
            .expect("dual emit call")
            .expect("dual emit ok");
        assert_eq!(result.emitted.len(), 2);
        assert_eq!(result.emitted[0].target.as_deref(), Some("application"));
        assert_eq!(result.emitted[1].target.as_deref(), Some("security"));
    }

    #[test]
    fn stopped_instance_does_not_allow_stale_log_emission() {
        let composition = Fabric::new("onoal.package.test.logging.lifecycle")
            .expect("fabric")
            .with(console_logging("application"))
            .with(log_emitter("application"))
            .build()
            .expect("composition");
        let mut instance = started_instance(
            &composition,
            "onoal.package.test.logging.lifecycle.instance",
        );
        {
            let emitter = activate::<LogEmitter>(&instance);
            block_on(emitter.emit_info("before stop".to_owned()))
                .expect("emit call")
                .expect("emit ok");
        }
        instance.stop().expect("stop");
        let stopped = instance.component::<LogEmitter>().expect("stopped emitter");
        assert!(block_on(stopped.emit_info("after stop".to_owned())).is_err());
    }

    #[test]
    fn console_formatting_is_deterministic_without_timestamp_claims() {
        let config = ConsoleLogConfig {
            minimum_level: LogLevel::Trace,
            stderr_from: LogLevel::Warn,
            prefix: Some("local".to_owned()),
        };
        assert_eq!(
            format_console_record(
                &config,
                &LogRecord::targeted(LogLevel::Info, "app", "hello")
            ),
            "local [INFO] app: hello"
        );
    }
}
