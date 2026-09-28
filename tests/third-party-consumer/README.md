# Third-Party Consumer Fixture

This crate is a public compatibility fixture for Fabric Ecosystem. It answers
whether code shaped like an independent downstream crate can use the public
Fabric, Package, Host, and Composition APIs without private repository
knowledge.

Run it from the repository root:

```text
cargo test -p fabric-ecosystem-third-party-consumer
```

The fixture lives under `tests/` because it is not a Package, Host,
Composition, Example, or Catalog artifact. Catalog generation deliberately
excludes `tests/*`, so this crate should not appear in `catalog/index.json`.

This fixture differs from examples: examples are runnable teaching
applications. The third-party consumer is a compatibility suite whose natural
entry point is `cargo test`.

This fixture also differs from package tests. Package tests prove package
semantics and edge cases; this crate proves representative external consumption
through public APIs only.

Covered families:

- data: KeyValue, RelationalDatabase with SQLite realization
- execution: ProcessRuntime run-to-completion invocation
- messaging and observability: Queue, Counter, Logging
- security: Ed25519 signing and verification
- networking: TCP transport and TCP inspection
- hosts: Linux Host detection and requirements
- compositions: HTTP Server and Local Backend reusable assemblies

The process-runtime scenario uses POSIX `sh` and is compiled only on Unix
targets. Linux Host detection tests are `cfg`-guarded for Linux and non-Linux
targets.

A failure here means a public downstream-style consumer can no longer express a
representative compatibility scenario with the documented ecosystem surface.
