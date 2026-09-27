# ProcessRuntime Package

`onoal-fabric-package-process-runtime` provides one Fabric Resource:

```text
ProcessRuntime
```

It represents a bounded capability to run one local OS process to completion and
return its completed outcome.

## Semantic Boundary

The process execution lifecycle is:

```text
ProcessInvocation
-> attempt to start child process
-> optional stdin bytes
-> wait for completion
-> ProcessOutput
```

This package does not model a long-lived process occurrence, process handle,
supervisor, daemon, worker pool, container runtime, scheduler, or deployment
system.

## ProcessInvocation

`ProcessInvocation` is the package-owned request model. It supports:

- `program`: executable/program name;
- `args`: argument vector;
- `working_directory`: optional child working directory;
- `environment`: environment variable overrides/additions;
- `stdin`: optional bytes written to the child process before waiting.

When no working directory is supplied, the child inherits the current process
working directory. When no environment entries are supplied, the child inherits
the current host process environment. Invocation environment entries override or
add variables for the child process; they do not clear the whole environment.

When stdin is absent, the local realization gives the child null/closed stdin.
When stdin is present, the bytes are written to child stdin before completion is
awaited. Streaming stdin is outside this package's v1 boundary.

## ProcessOutput

`ProcessOutput` represents a child process that actually started and completed.
It contains:

- portable numeric exit code when available;
- captured stdout bytes;
- captured stderr bytes.

Raw bytes are canonical. `stdout_utf8()` and `stderr_utf8()` are lossy display
helpers.

A child that exits with code `1` is still a completed process outcome:

```text
Ok(ProcessOutput { status_code: Some(1), ... })
```

It is not an execution error.

## ProcessExecutionError

`ProcessExecutionError` represents failure to perform the execution operation,
such as:

- runtime not started/stopped;
- spawn failure;
- stdin write failure;
- wait failure.

Implementation-specific `std::io::Error` values do not leak through the public
semantic API.

## LocalProcessRuntime

`LocalProcessRuntime` is the first realization. It uses the local operating
system's process APIs to spawn a child, apply working directory and environment
overrides, optionally pipe stdin, capture stdout/stderr, and wait for
completion.

Output is captured fully in memory. This package is not suitable for
indefinitely running processes or unbounded output streams.

## Authoring

Use `local_process_runtime` to contribute one named local realization:

```rust
use fabric::prelude::*;
use fabric_package_process_runtime::local_process_runtime;

let composition = Fabric::new("example")?
    .with(local_process_runtime("local"))
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Consumer-owned Components should require `ProcessRuntime` directly and define
their own execution behavior.

## Host Relationship

Fabric owns Host truth through `HostDescriptor`, `HostOperatingSystem`, and
`HostArchitecture`. This package does not recreate OS or architecture facts.

A future realization may declare Host requirements when real compatibility
pressure exists.

## Extension Path

Future pressure may introduce separate capabilities for living process handles,
termination, streaming I/O, timeout policy, resource limits, supervision, worker
runtimes, or containers.

## Non-Goals

This package deliberately does not model:

- long-running `ProcessHandle`;
- streaming I/O;
- process supervision;
- restart policy;
- daemon/service lifecycle;
- worker pools;
- containers or OCI;
- Deno;
- deployment;
- scheduling;
- Oracle concepts;
- remote execution.
