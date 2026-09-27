# KeyValue Package

`onoal-fabric-package-key-value` provides one Fabric Resource:

```text
KeyValue
```

It represents a named key/value store capability with UTF-8 textual keys and
opaque byte values.

## Semantic Ownership

`KeyValue` owns three operations:

- `get(key) -> Result<Option<Vec<u8>>, KeyValueError>`
- `set(key, value) -> Result<(), KeyValueError>`
- `delete(key) -> Result<Option<Vec<u8>>, KeyValueError>`

Missing keys are normal data state, not operation failure:

- `get(missing)` returns `Ok(None)`.
- `delete(missing)` returns `Ok(None)`.

`set` is unconditional insert-or-replace. It does not imply compare-and-set,
transactions, leases, or conditional mutation.

## Error Boundary

Operations return package-owned `KeyValueError` values so future file-backed,
database-backed, device-backed, or remote realizations can fail honestly without
leaking implementation-specific errors.

Fabric lifecycle availability remains Fabric truth. `KeyValueError` describes a
live realization failing to complete a read, write, or delete operation.

## Memory Realization

`MemoryKeyValue` is the first realization. It stores values in generation-local
memory.

That means:

- data is non-durable;
- a fresh Fabric Instance generation starts empty;
- multiple named `KeyValue` occurrences have isolated state;
- internal storage and synchronization are not public API.

## Authoring

Use `memory_key_value` to contribute one in-memory occurrence:

```rust
use fabric::prelude::*;
use fabric_package_key_value::memory_key_value;

let composition = Fabric::new("example")?
    .with(memory_key_value("primary"))
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Consumer-owned Components should require `KeyValue` directly and define their
own behavior. The package does not ship generic scripted client Components.

## Augmentation

Other crates may define augmentations that attach to `KeyValue` through normal
Fabric augmentation APIs. This package does not claim audit semantics. A real
audit capability would need its own durable record model, attribution boundary,
retention guarantees, and enforcement story.

## Extension Path

Future work may add additional realizations beside `MemoryKeyValue`, such as
file-backed or database-backed stores, without changing the `KeyValue` semantic
identity.

## Non-Goals

`KeyValue` v1 deliberately does not model:

- TTL or cache expiry;
- transactions;
- compare-and-set;
- batch APIs;
- scan/list/query APIs;
- ordered database semantics;
- serialization, schemas, JSON, or typed values;
- distributed replication;
- durability;
- audit/history semantics.
