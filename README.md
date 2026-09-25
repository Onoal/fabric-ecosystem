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
  |-- compositions/  reusable assembled systems
  |-- examples/      teaching and demonstrations
  |-- tests/         compatibility and integration verification
  `-- docs/          architecture and repository guidance
```

- `packages/data/key-value`: a semantic KeyValue resource with a stateful in-memory
  adapter and reusable authoring contributions.
- `packages/execution/process-runtime`: a local process execution capability with a
  real OS-process adapter, execution environment system, and component witness.
- `packages/messaging/queue`: a bounded non-durable FIFO queue capability with
  in-memory live state and producer/consumer component witnesses.
- `packages/networking/tcp`: a loopback TCP byte-stream transport capability with real
  OS bind/connect/accept/read/write behavior and Component probes.
- `packages/security/ed25519`: an Ed25519 signing capability with ephemeral
  live private-key state, public signature evidence, and Component witnesses.

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

`compositions/` is reserved for coherent reusable assembled systems. It is not
a package bucket and not a second Fabric runtime primitive.

`examples/` contains learning artifacts. Examples may be explicit and
pedagogical; they are not compatibility fixtures.
