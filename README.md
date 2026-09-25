# Fabric Packages

Reusable Fabric package crates organized by capability family.

This repository is a distribution and source-organization layer on top of
Fabric 0.7. A package is not a Fabric semantic primitive: package boundaries do
not appear in `Composition` inspection, do not create runtime lifecycle, and do
not introduce package IDs inside Fabric.

The first package families are:

- `data/key-value`: a semantic KeyValue resource with a stateful in-memory
  adapter and reusable authoring contributions.
- `execution/process-runtime`: a local process execution capability with a
  real OS-process adapter, execution environment system, and component witness.
- `messaging/queue`: a bounded non-durable FIFO queue capability with
  in-memory live state and producer/consumer component witnesses.
- `networking/tcp`: a loopback TCP byte-stream transport capability with real
  OS bind/connect/read/write behavior and Component probes.

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
