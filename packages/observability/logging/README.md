# Logging

`LogSink` is a semantic logging capability for Fabric applications and systems
that explicitly emit log records through ordinary Resource relations.

It is not a global process logger and no Composition is required to include
logging.

## Model

`LogRecord` contains:

- `level`: `Trace`, `Debug`, `Info`, `Warn`, or `Error`
- `message`: producer-owned text
- `target`: optional logical producer/category target

The level ordering is:

```text
Trace < Debug < Info < Warn < Error
```

`target` is ordinary logging categorization. It is not identity, authority,
audit subject, service identity, or provenance.

There is no timestamp in the semantic contract. A future timestamp model must
earn a clock/observer guarantee instead of borrowing ambient system time.

`LogError` is the package-owned operation error boundary. Console write failures
map to `LogErrorKind::WriteFailed`; `std::io::Error` is not exposed.

## Console Realization

`ConsoleLogSink` is the first local realization. `ConsoleLogConfig` is
realization-owned and controls:

- `minimum_level`: records below this level are accepted but filtered
- `stderr_from`: records at or above this level are written to stderr
- `prefix`: optional local output prefix

By default, `Trace`, `Debug`, and `Info` go to stdout, while `Warn` and `Error`
go to stderr.

Console output is one record per line. Prefix, target, and message carriage
returns/newlines are escaped as `\r` and `\n`, so one emitted `LogRecord` cannot
silently forge additional console record lines.

## Consumer-Owned Behavior

Consumers own what gets logged, when records are emitted, which targets are
used, and any multi-sink policy. A Component should require `LogSink` directly
and call `emit(LogRecord)`.

Multiple named sinks such as `application` and `security` are ordinary Resource
occurrences. There is no hidden singleton.

## Boundaries

`LogSink` is not `CounterMetric`.

```text
counter: "12 requests occurred"
log:     "request failed because ..."
```

Logging is not Fabric Observation, Fabric lifecycle diagnostics, tracing, audit
evidence, history, identity, or durable proof. Console output is ephemeral
operational output with no retention, tamper resistance, legal evidence, or
authoritative history guarantees.

This package does not install a `log` crate global logger or a `tracing` global
subscriber. It models explicit Fabric Resource participation.

## Extension Path

Future realizations may write files, JSON, journald, OpenTelemetry exporters, or
remote logging services. Future semantic work may add structured fields if real
pressure earns them. Those extensions do not change the current `LogSink`
boundary.

## Non-Goals

- Fabric Instance observation
- tracing spans or trace/span correlation
- audit evidence/history
- durable retention
- identity-aware logs
- timestamps with clock guarantees
- structured field schema
- global logger/subscriber bridge
- file rotation
- async buffering
- fanout, sampling, or rate limiting policy
