# SQLite RelationalDatabase Realization

This package realizes the `RelationalDatabase` Resource from
`onoal-fabric-package-relational-database` using real SQLite.

`RelationalDatabase` remains the semantic owner. SQLite is the concrete Adapter
and owns SQLite-specific configuration.

## Config

The Adapter supports:

- file-backed databases
- in-memory databases

File-backed databases are the durable path. The SQLite file is external durable
data, not Fabric Composition truth and not generic Instance observation.

## Lifecycle

Starting a Fabric Instance opens the SQLite connection. Stopping the generation
closes it so a fresh generation can reopen the same file. Rows stored in the
file remain external database state across Fabric generations.

## Boundary

SQLite-specific types such as `rusqlite::Connection`, `rusqlite::Row`,
`rusqlite::Value`, and `rusqlite::Error` are not exposed through the generic
semantic API. Errors are translated into the package-owned
`RelationalDatabaseError` surface.

SQLite-specific SQL behavior is not promoted to generic relational database
semantics. Future PostgreSQL or third-party realizations should target the same
`RelationalDatabase` Resource without changing this semantic package.
