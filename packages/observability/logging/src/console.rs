use std::io::{self, Write};

use crate::{LogError, LogLevel, LogRecord, LogSink};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConsoleDestination {
    Stdout,
    Stderr,
}

fn destination(config: &ConsoleLogConfig, level: LogLevel) -> ConsoleDestination {
    if level >= config.stderr_from {
        ConsoleDestination::Stderr
    } else {
        ConsoleDestination::Stdout
    }
}

fn escape_line_breaks(value: &str) -> String {
    value.replace('\r', "\\r").replace('\n', "\\n")
}

fn format_console_record(config: &ConsoleLogConfig, record: &LogRecord) -> String {
    let prefix = config.prefix.as_deref().map(escape_line_breaks);
    let target = record.target.as_deref().map(escape_line_breaks);
    let message = escape_line_breaks(&record.message);
    let target = target
        .as_ref()
        .map(|target| format!(" {target}:"))
        .unwrap_or_default();
    match prefix {
        Some(prefix) => format!("{prefix} [{}]{target} {message}", record.level),
        None => format!("[{}]{target} {message}", record.level),
    }
}

fn write_line(writer: &mut impl Write, line: &str) -> Result<(), LogError> {
    writer
        .write_all(line.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .map_err(|error| LogError::write_failed(error.to_string()))
}

fn write_console_record(
    config: &ConsoleLogConfig,
    record: &LogRecord,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<(), LogError> {
    if record.level < config.minimum_level {
        return Ok(());
    }
    let line = format_console_record(config, record);
    match destination(config, record.level) {
        ConsoleDestination::Stdout => write_line(stdout, &line),
        ConsoleDestination::Stderr => write_line(stderr, &line),
    }
}

fabric::adapter! {
    pub ConsoleLogSink for LogSink {
        id: "onoal.package.observability.logging.console";

        config {
            config: ConsoleLogConfig;
        }

        runtime {
            async fn emit(&self, record: LogRecord) -> Result<(), LogError> {
                let mut stdout = io::stdout().lock();
                let mut stderr = io::stderr().lock();
                write_console_record(&self.config.config, &record, &mut stdout, &mut stderr)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("writer failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn formatting_covers_prefix_target_levels_and_control_policy() {
        assert_eq!(
            format_console_record(
                &ConsoleLogConfig::default(),
                &LogRecord::new(LogLevel::Info, "hello")
            ),
            "[INFO] hello"
        );
        assert_eq!(
            format_console_record(
                &ConsoleLogConfig {
                    prefix: Some("local".to_owned()),
                    ..ConsoleLogConfig::default()
                },
                &LogRecord::new(LogLevel::Debug, "hello")
            ),
            "local [DEBUG] hello"
        );
        assert_eq!(
            format_console_record(
                &ConsoleLogConfig::default(),
                &LogRecord::targeted(LogLevel::Warn, "app", "hello")
            ),
            "[WARN] app: hello"
        );
        assert_eq!(
            format_console_record(
                &ConsoleLogConfig {
                    prefix: Some("pre\nfix".to_owned()),
                    ..ConsoleLogConfig::default()
                },
                &LogRecord::targeted(LogLevel::Error, "tar\rget", "line\nbreak")
            ),
            "pre\\nfix [ERROR] tar\\rget: line\\nbreak"
        );
    }

    #[test]
    fn level_filtering_and_destination_are_deterministic() {
        let config = ConsoleLogConfig {
            minimum_level: LogLevel::Info,
            stderr_from: LogLevel::Warn,
            prefix: None,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        write_console_record(
            &config,
            &LogRecord::new(LogLevel::Debug, "filtered"),
            &mut stdout,
            &mut stderr,
        )
        .expect("filtered records are accepted");
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());

        write_console_record(
            &config,
            &LogRecord::new(LogLevel::Info, "out"),
            &mut stdout,
            &mut stderr,
        )
        .expect("stdout write");
        write_console_record(
            &config,
            &LogRecord::new(LogLevel::Warn, "err"),
            &mut stdout,
            &mut stderr,
        )
        .expect("stderr write");

        assert_eq!(String::from_utf8(stdout).expect("stdout"), "[INFO] out\n");
        assert_eq!(String::from_utf8(stderr).expect("stderr"), "[WARN] err\n");
        assert_eq!(
            destination(&config, LogLevel::Trace),
            ConsoleDestination::Stdout
        );
        assert_eq!(
            destination(&config, LogLevel::Error),
            ConsoleDestination::Stderr
        );
    }

    #[test]
    fn writer_failure_maps_to_write_failed() {
        let config = ConsoleLogConfig::default();
        let mut stdout = FailingWriter;
        let mut stderr = Vec::new();
        let error = write_console_record(
            &config,
            &LogRecord::new(LogLevel::Info, "hello"),
            &mut stdout,
            &mut stderr,
        )
        .expect_err("writer failure");

        assert_eq!(error.kind, crate::LogErrorKind::WriteFailed);
    }
}
