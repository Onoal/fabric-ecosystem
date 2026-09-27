# KeyValue Quickstart Example

This is the smallest runnable Fabric Ecosystem example. It materializes an
in-memory `KeyValue` store, runs an example-owned Component that requires the
`KeyValue` Resource, writes a value, reads it, deletes it, and confirms the key
is missing afterward.

Run from the repository root:

```sh
cargo run -p fabric-ecosystem-example-key-value-quickstart
```

Expected output includes:

```text
stored: fabric
deleted: fabric
after delete: missing
```

Uses:

- `fabric-package-key-value`
- `memory_key_value("primary")`

The application behavior belongs to this example. It does not introduce a
reusable KeyValue client, cache, database abstraction, or Composition.

Source map:

- `src/main.rs`: runnable scenario, example-owned Component, and visible output.
