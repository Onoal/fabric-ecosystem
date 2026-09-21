# Fabric packages

This repository contains independently reusable capability packages built
against the published Fabric crate contract.

Current package lines:

- `packages/resource-key-value/` — `onoal-fabric-resource-key-value`, the
  semantic KeyValue Resource.
- `adapters/key-value-memory/` — an independent in-memory KeyValue
  realization.
- `adapters/key-value-filesystem/` — an independent filesystem KeyValue
  realization.
- `verification/key-value-consumer/` — executable proof that one consumer can
  use either KeyValue realization and that named occurrences remain isolated.

- `process/` — `onoal-fabric-process`, a reusable local operating-system
  process capability for Fabric compositions. It remains the current
  lower-level implementation line; it is not yet the canonical ProcessRuntime
  semantic package.
