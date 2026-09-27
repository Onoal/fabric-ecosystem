//! Semantic logging capability for Fabric.
//!
//! Logging is explicit application/system behavior that emits log records
//! through a named `LogSink` Resource. It is not Fabric Instance observation,
//! lifecycle diagnostics, tracing, audit evidence, history, identity, or
//! durable proof.

mod authoring;
mod console;
mod error;
mod model;
mod resource;

pub use authoring::{console_logging, console_logging_with};
pub use console::{ConsoleLogConfig, ConsoleLogSink, ConsoleLogSinkConfig};
pub use error::{LogError, LogErrorKind};
pub use model::{LogLevel, LogRecord};
pub use resource::LogSink;
