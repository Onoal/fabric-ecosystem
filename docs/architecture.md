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
