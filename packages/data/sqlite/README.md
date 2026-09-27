# SQLite RelationalDatabase Realization

`onoal-fabric-package-sqlite` realizes the `RelationalDatabase` Resource from
`onoal-fabric-package-relational-database` using real SQLite.

`RelationalDatabase` remains the semantic owner. This package owns only the
SQLite-specific Adapter, config, lifecycle, value conversion, error
translation, and authoring helpers.

## Config

`SqliteDatabasePath` is realization-specific config:

- `File(PathBuf)`;
- `InMemory`.

File-backed databases are external durable data. In-memory databases are
generation-local SQLite state.

## Parent Directory Policy

For file-backed databases, startup creates the parent directory with
`create_dir_all` before opening the SQLite file. This is an intentional local
authoring convenience of the SQLite realization.

The database file and directory are not Fabric Composition truth.

## Lifecycle

Starting a Fabric Instance opens the SQLite connection. Stopping the generation
closes it so a fresh generation can reopen the same file.

Open/create failures during start and close failures during stop are Adapter
lifecycle failures and surface as Fabric `ModuleError` values. Statement
execution/query failures after start translate to `RelationalDatabaseError`.

## State and Concurrency

Each SQLite Resource occurrence owns one synchronized SQLite connection.

This package does not provide connection pooling, multi-connection concurrency,
read replicas, or remote database management.

## Conversion Boundary

The realization translates:

```text
RelationalValue <-> rusqlite::types::Value
```

internally. Public APIs do not expose `rusqlite::Connection`, `rusqlite::Row`,
`rusqlite::Value`, or `rusqlite::Error`.

## Persistence

Rows stored in a file-backed SQLite database persist across Fabric generations:

```text
generation A writes file
generation A stops
generation B opens same file
data remains
```

Rows in `InMemory` mode do not persist across generations.

## Authoring

Useful helpers are:

- `sqlite_database(name, path)`
- `in_memory_sqlite_database(name)`
- `sqlite_database_with(name, SqliteDatabasePath)`

These helpers lower to ordinary Fabric contribution truth.

## Transactions

M9G does not introduce a structured transaction API. Raw `BEGIN`/`COMMIT`
statements may be sent as backend-understood statement text, but this package
does not claim safe structured transaction ownership across concurrent
consumers.

## Non-Goals

This package deliberately does not model:

- generic relational semantics;
- ORM behavior;
- migration systems;
- connection pooling;
- replication;
- remote SQLite services;
- transaction managers;
- schema management.
