# Logging

`LogSink` is a semantic logging capability for Fabric applications and systems
that explicitly want to emit log records.

It is a Resource because a Component may require a named logging capability and
emit records through an ordinary Fabric relation. It is not a global process
logger and not every Composition must include logging.

## Log Model

The package-owned model is intentionally small:

- `LogLevel`: `Trace`, `Debug`, `Info`, `Warn`, `Error`
- `LogRecord`: `level`, `message`, and an optional `target`
- `LogError`: bounded package-level failures

There is no timestamp in the semantic contract. A realization may include local
time in its output later, but Fabric does not provide authoritative distributed
time.

## Console Realization

`ConsoleLogSink` is the first local realization. It writes real process output:

- `Trace`, `Debug`, and `Info` use stdout by default.
- `Warn` and `Error` use stderr by default.

Console configuration belongs to the Adapter. It may set a minimum level, the
level where stderr begins, and an optional prefix. These are local output
choices, not new logging semantics.

## Boundaries

`LogSink` is not `CounterMetric`.

```text
counter: "12 requests occurred"
log:     "request failed because ..."
```

Logging is also not Fabric lifecycle diagnostics, Fabric Instance observation,
internal tracing, Oracle Observation, Origin/Identis identity history, or audit
evidence.

A console log record is ephemeral operational output. It does not provide
durability, tamper resistance, retention guarantees, identity attribution,
legal evidence, or authoritative history.

Future realizations such as files, journald, OpenTelemetry exporters, or remote
logging services can realize `LogSink` without changing the semantic Resource.
