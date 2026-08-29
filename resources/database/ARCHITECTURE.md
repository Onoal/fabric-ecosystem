# Fabric Database Resource

Database is a Resource.

Frozen E2 laws:

- Database is one canonical Resource box
- `ResourceId("database")` is stable across Database adapters
- Database owns `DatabaseRef` and Database Resource Instance semantics
- `DatabaseRef` identifies a Database Resource Instance, not an SQLite file
- Database configuration is Database-owned
- Database binding and inspection meaning are Database-owned
- SQLite is the Beta Database adapter
- SQLite != Database
- adapter-specific materialization is not canonical Database semantic truth
- safe generic Database inspection must not expose provider topology such as
  absolute host paths or SQLite handles

Database owns:

- stable Database Resource identity via `database_resource_id()`
- `DatabaseRef`
- prepare / cleanup / reference / bind semantics
- Database-owned configuration via `DatabaseConfig`
- Database-owned inspection semantics
- Database-owned adapter selection and topology
- Database-specific binding specialization on top of the shared Resource Binding rail

Shared Fabric resource grammar still owns:

- `ResourceInstanceId`
- `ResourceContext`
- `ResourceName`
- shared instance identity encoding

SQLite lives below the Database Resource boundary and owns:

- SQLite file layout
- SQLite connection mechanics
- pragmas
- WAL
- busy timeout
- file-backed initialization and verification

E8 closes Database configuration and safe inspection ownership without changing
the Database Resource model:

- constructor `DatabaseConfig` remains initial Database Resource supply
- the same Database-owned configuration consumer owns the effective root used by
  runtime behavior
- replaying the same root is idempotent
- changing the configured root after initialization is rejected atomically
- generic Resource safe inspection exposes only non-sensitive Database facts such
  as adapter and configured state
- provider-specific filesystem topology remains below the Database boundary

Current Worker/Deno compatibility still needs a concrete SQLite-specific
Database-owned compatibility seam:

- `SqliteDatabaseCompatibilityContract`
- `SqliteDatabaseMaterialization`

That seam is temporary Database-owned compatibility for the current Projection
rail adapters that specifically need SQLite backing information. It is not
canonical Database identity, not generic Resource projection truth, and not a
Resource Registry concern.

After E3:

- `DatabaseBinding` is explicit relationship truth
- cross-context binding preserves the same underlying `DatabaseRef`
- binding an existing `DatabaseRef` does not reprovision another Database instance
- `BindingId` now owns shared binding identity above Database-specific meaning

After E4:

- `WorkloadBindingProjection` is downstream consumer policy, not Database truth
- Projection coordinates late consumer-ready materialization
- Database still owns how a `DatabaseRef` becomes current SQLite-backed
  compatibility material
- Deno no longer resolves Database meaning directly
- adapter-specific compatibility material remains below the Database
  Resource boundary even when a current consumer needs it
