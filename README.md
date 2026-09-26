# Fabric Ecosystem

Shareable Fabric artifacts: reusable packages, future reusable compositions,
and runnable examples.

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
  |-- examples/      teaching and demonstrations
  |-- tests/         compatibility and integration verification
  |-- catalog/       discovery and navigation contract
  `-- docs/          architecture and repository guidance
```

- `packages/data/key-value`: a semantic KeyValue resource with a stateful in-memory
  adapter and reusable authoring contributions.
- `packages/data/relational-database`: generic bounded relational database
  semantics with portable value/result/error types.
- `packages/data/sqlite`: a SQLite realization of `RelationalDatabase` with
  file-backed local persistence.
- `packages/execution/process-runtime`: a local process execution capability with a
  real OS-process adapter, execution environment system, and component witness.
- `packages/messaging/queue`: a bounded non-durable FIFO queue capability with
  in-memory live state and producer/consumer component witnesses.
- `packages/networking/tcp`: a loopback TCP byte-stream transport capability with real
  OS bind/connect/accept/read/write behavior and Component probes.
- `packages/networking/http`: HTTP/1 server behavior layered over the TCP
  byte-stream capability.
- `packages/observability/counter`: a monotonic counter metric capability with
  in-memory live telemetry state and instrumentation Component witnesses.
- `packages/observability/logging`: a semantic LogSink capability with a local
  console realization for explicit application/system log records.
- `packages/security/ed25519`: an Ed25519 signing capability with ephemeral
  live private-key state, public signature evidence, and Component witnesses.
- `hosts/linux`: Linux Host detection, Linux facility identifiers, and reusable
  Host requirements using Fabric's existing Host model.
- `compositions/web/http-server`: the first reusable Composition artifact,
  assembling TCP loopback realization, TCP runtime address discovery, and
  HTTP/1 server behavior.
- `compositions/web/local-backend`: a nested local backend foundation that
  reuses the HTTP Server Composition and adds SQLite persistence plus Console
  logging.
- `compositions/messaging/local-queue-pipeline`: a local messaging foundation
  that assembles one named in-memory FIFO queue with one producer and one
  consumer.

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
existing Host model. Host artifacts are not Resources, Systems, Components, or
Adapters.

`compositions/` contains coherent reusable assembled systems. It is not a
package bucket and not a second Fabric runtime primitive.

`examples/` contains learning artifacts. Examples may be explicit and
pedagogical; they are not compatibility fixtures.

`tests/` contains cross-package integration, compatibility, and third-party
public-consumer verification.

`catalog/` is reserved for generated or curated discovery views. Catalog
metadata helps humans and tools find artifacts; it does not duplicate Fabric
semantic truth.
