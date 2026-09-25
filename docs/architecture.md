# Fabric Package Architecture

Fabric packages are reusable Rust crates that publish ordinary Fabric
definitions and contribution helpers.

They are not Fabric semantic truth. A package may define resources, systems,
components, adapters, augmentations, and `FabricContribution` helpers, but after
authoring the resulting `Composition` contains only Fabric semantic truth:
resources, systems, components, relations, realizations, augmentations, and
live instance observations.

## Topology

Packages are grouped by capability ownership:

- `data/key-value` owns the KeyValue capability.
- `execution/process-runtime` owns local process execution.

This repository deliberately avoids kind-based folders such as `resources/` or
`adapters/`. Those folders would make implementation machinery look more
important than capability ownership.

## Reserved Capability Pressure

The first foundation is intentionally breadth-first, not exhaustive.

- Network and transport packages should own real connectivity capabilities,
  not merely wrap process execution or key-value state.
- Messaging packages should model delivery semantics, queues, streams, or
  routing as their own capability family when real behavior exists.
- Security and crypto packages should expose concrete cryptographic or
  authorization capabilities without borrowing identity concepts from other
  Onoal systems.
- Observability packages should own telemetry, metrics, traces, or audit
  behavior when those capabilities are implemented as real Fabric definitions
  and contributions.

None of these reserved families should appear as empty crates. A package family
is introduced only when it contains useful Fabric authoring.

## Fabric Boundary

Fabric 0.7 remains unchanged. Package helpers use public Fabric APIs:

- `resource!`
- `system!`
- `component!`
- `adapter!`
- `FabricContribution`
- `Composition` inspection
- `Instance` observation and typed invocation

No package introduces `Package`, `PackageId`, `PackageManifest`, or package
runtime state into Fabric itself.
