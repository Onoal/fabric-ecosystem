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
- `delete(key) -> Result<(), KeyValueError>`

All three operations are Fabric Resource operations and are therefore invoked
through the uniform awaitable Resource boundary:

```rust
let value = store.get("session".to_owned()).await?;
store.set("session".to_owned(), b"active".to_vec()).await?;
store.delete("session".to_owned()).await?;
# Ok::<(), KeyValueError>(())
```

Missing keys are normal data state, not operation failure:

- `get(missing)` returns `Ok(None)`.
- `delete(missing)` succeeds with `Ok(())`.

`set` is unconditional insert-or-replace. It does not imply compare-and-set,
transactions, leases, or conditional mutation.

`delete` is an idempotent removal operation. It does not return the previous
value, an existence boolean, or a row count.

## Base Guarantees

`KeyValue` owns the capability semantics for `get`, `set`, and `delete`. A
realization owns its actual visibility, durability, and environment
characteristics.

Awaiting `set` or `delete` means the realization reports successful completion
of that operation. Base `KeyValue` does not promise durability, global
immediate visibility, read-your-writes across distributed locations,
transactions, compare-and-set, TTL, or replication behavior.

Any realization claiming `KeyValue` must implement the complete base
`get`/`set`/`delete` contract truthfully. A realization may have stronger
behavior that this base Resource does not expose.

## Breaking Change

The `delete` operation now returns completion only:

```rust
store.delete("session".to_owned()).await?;
```

Callers that relied on the removed value must perform a separate read only
when their own application semantics genuinely require one. Such a read is not
atomic with delete and must not be treated as delete-and-return semantics.

## Error Boundary

Operations return package-owned `KeyValueError` values so realizations can fail
honestly without leaking implementation-specific errors.

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
