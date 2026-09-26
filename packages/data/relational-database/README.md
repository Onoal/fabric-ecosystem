# RelationalDatabase

`RelationalDatabase` is the generic relational database capability package for
Fabric.

It owns one semantic Resource: a named relational database occurrence through
which a consumer can perform a bounded set of SQL-style operations.

## API

The Resource exposes:

```text
execute(statement, parameters) -> changed row count
query(statement, parameters) -> rows
```

Parameters and returned values use package-owned portable types:

- `Null`
- `Integer`
- `Real`
- `Text`
- `Bytes`

The result model is deliberately small. Rows are ordered lists of values; this
package is not an ORM and does not model entities.

## SQL Boundary

The API is portable. Arbitrary SQL text is not guaranteed to be portable across
all future database realizations. Package tests use intentionally simple SQL so
that a future PostgreSQL realization can pressure the same semantic surface.

This package does not provide a SQL parser, query builder, schema migration
framework, or Fabric-owned database language.

## Realizations

Concrete backends realize this Resource from separate crates. For example:

```text
RelationalDatabase
├── SQLite
├── future PostgreSQL-related realization
└── future third-party Adapter
```

A realization may expose its own Adapter Config. Backend-specific facts such as
database file paths or cloud provider settings do not belong to the generic
Resource Config.

## Non-goals

This package deliberately does not model:

- ORM behavior
- schema migrations
- connection pools
- replication
- backups
- distributed database semantics
- transactions beyond what the current API exposes
- PostgreSQL-specific semantics
- SQLite-specific semantics
- cloud provider semantics
