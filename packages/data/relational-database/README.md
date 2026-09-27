# RelationalDatabase Package

`onoal-fabric-package-relational-database` provides one Fabric Resource:

```text
RelationalDatabase
```

It represents a named relational database capability through which consumers
can execute backend-understood relational statements and query portable rows and
values.

## Semantic Boundary

The Resource exposes:

```text
execute(statement, parameters) -> Result<changed row count, RelationalDatabaseError>
query(statement, parameters)   -> Result<RelationalQueryResult, RelationalDatabaseError>
```

`execute` is for statements that do not return a row result set. `query` is for
statements that return rows.

## Statement / Dialect Law

This package owns the portable invocation and result shape. It does not own a
portable SQL dialect.

Statement text is interpreted by the selected realization/backend. Tests use
simple SQL deliberately, but arbitrary vendor-specific SQL strings are not
guaranteed to work across all future realizations.

This package does not provide a SQL parser, query builder, schema DSL, or
Fabric-owned query language.

## Portable Values

Parameters and returned values use a small package-owned intersection:

- `Null`
- `Integer(i64)`
- `Real(f64)`
- `Text(String)`
- `Bytes(Vec<u8>)`

Backend-specific types such as SQLite affinities, PostgreSQL UUIDs, JSONB,
dates, timestamps, decimals, and arrays require future semantic pressure.

## Query Results

`RelationalQueryResult` contains ordered `RelationalRow` values. Rows currently
expose positional values only.

Column metadata is deliberately deferred for v1. Positional rows are sufficient
for the current bounded capability because consumers control the statement they
send and can select columns in a known order. Portable column identity, declared
types, and schema introspection require separate pressure.

## Error Boundary

Operation errors are package-owned:

- `ExecuteFailed`
- `QueryFailed`
- `NotStarted`
- `Stopped`

Open/start failures belong to Adapter lifecycle and surface as Fabric lifecycle
errors, not `RelationalDatabaseError`. A live realization translates statement
execution/query failures into `RelationalDatabaseError` without leaking backend
implementation error types.

## Realizations

Concrete backends realize this Resource from separate crates. For example:

```text
RelationalDatabase
├── SQLite
├── future PostgreSQL-related realization
└── future third-party Adapter
```

Backend-specific facts such as database file paths, in-memory mode, remote
connection strings, or cloud provider settings belong to realization config, not
to the generic Resource.

## Consumer Pattern

Consumer-owned Components should require `RelationalDatabase` directly and own
their own application/database behavior.

## Non-Goals

This package deliberately does not model:

- ORM behavior;
- schema migrations;
- query builders;
- portable SQL dialect normalization;
- structured transaction ownership;
- connection pools;
- distributed database semantics;
- database discovery;
- SQLite-specific semantics.
