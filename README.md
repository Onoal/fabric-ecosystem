# Fabric Ecosystem

Shareable Fabric artifacts: reusable packages, reusable compositions, Host
artifacts, runnable operational Instance entrypoints, runnable examples,
compatibility fixtures, and a generated Catalog.

Fabric itself provides the semantic systems-construction model. Fabric
Ecosystem contains source artifacts built with Fabric.

```text
Fabric
  |
  v
Fabric Ecosystem
  |-- packages/      reusable capability building material
  |-- hosts/         reusable Host/environment artifacts
  |-- compositions/  reusable assembled systems
  |-- instances/     runnable operational Instance entrypoints
  |-- examples/      runnable teaching applications
  |-- tests/         compatibility and integration verification
  |-- catalog/       discovery and navigation contract
  `-- docs/          architecture and repository guidance
```

- `packages/data/key-value`: a fallible textual-key/opaque-byte `KeyValue`
  capability with a generation-local in-memory realization.
- `packages/data/relational-database`: generic bounded relational database
  semantics with portable value/result/error types.
- `packages/data/sqlite`: a SQLite realization of `RelationalDatabase` with
  file-backed local persistence.
- `packages/execution/process-runtime`: a run-to-completion local process
  execution capability with invocation, output, and error models.
- `packages/messaging/queue`: a bounded non-durable FIFO queue capability with
  generation-local in-memory realization.
- `packages/networking/tcp`: a loopback TCP byte-stream transport capability with real
  OS bind/connect/accept/read/write behavior and runtime inspection.
- `packages/networking/http`: HTTP/1 server behavior layered over the TCP
  byte-stream capability.
- `packages/observability/counter`: a monotonic counter metric capability with
  in-memory live telemetry state.
- `packages/observability/logging`: a semantic LogSink capability with a local
  console realization for explicit application/system log records.
- `packages/security/ed25519`: an Ed25519 signing capability with ephemeral
  live private-key state and public signature evidence.
- `hosts/linux`: Linux Host detection, Linux facility identifiers, and reusable
  Host requirements using Fabric's existing Host model.
- `compositions/web/http-server`: the first reusable Composition artifact,
  assembling TCP loopback realization, TCP runtime address discovery, and
  HTTP/1 server behavior.
- `compositions/web/local-backend`: a nested local backend foundation that
  reuses the HTTP Server Composition and adds SQLite persistence plus Console
  logging.
- `instances/web/http-server`: the first long-running operational Instance
  artifact, running the HTTP Server Composition as a live `fabric::Instance`.
Messaging currently provides the `FifoQueue` package capability. A reusable
messaging Composition will be added only when multiple genuinely useful
messaging/runtime behaviors form a coherent assembled system.

Packages expose ordinary Rust functions returning Fabric contributions, so a
consumer writes normal Fabric authoring:

```rust
use fabric::prelude::*;
use fabric_package_key_value::memory_key_value;

let composition = Fabric::new("example")?
    .with(memory_key_value("primary"))
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Package topology is domain-first. It is intentionally not split into
`resources/`, `systems/`, `components/`, or `adapters/`, because a real package
may contain all of those Fabric authoring forms.

`hosts/` is reserved for reusable environment artifacts built on Fabric's
existing Host model. Host artifacts project evidence into ordinary
`HostDescriptor` / `HostRequirement` truth; they are not Resources, Systems,
Components, Adapters, or parallel Host ontologies.

`compositions/` contains coherent reusable assembled systems. It is not a
package bucket and not a second Fabric runtime primitive.

`instances/` contains runnable operational entrypoints. An Instance artifact is
source that materializes and operates a live `fabric::Instance`; it is not the
live runtime Instance itself, persisted runtime state, or deployment policy.

`examples/` contains runnable teaching applications. Examples may be explicit
and pedagogical; they are not compatibility fixtures or reusable package APIs.

`tests/` contains cross-package integration, compatibility, and third-party
public-consumer verification.

`catalog/` contains generated discovery views:

- [catalog/index.md](catalog/index.md) for human navigation.
- [catalog/index.json](catalog/index.json) for machine-readable discovery.

Catalog metadata helps humans and tools find artifacts; it does not duplicate
Fabric semantic truth.

## Verification

The repository has one canonical local verification sequence. Run it from the
repository root before accepting a change.

The sequence uses the committed `Cargo.lock` for Cargo commands that resolve
dependencies:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked -p fabric-ecosystem-catalog-tool -- check
cargo run --locked -p fabric-ecosystem-catalog-tool -- validate
```

`cargo test --workspace --locked` includes the third-party public compatibility
fixture and the example binary smoke tests. Catalog `check` proves the committed
projection is current with repository source truth; Catalog `validate` proves
the committed machine document satisfies the strict Catalog v2 contract.

GitHub-hosted CI is not part of the repository verification model. GitHub may
host the remote repository, but verification is an explicit local contract.
