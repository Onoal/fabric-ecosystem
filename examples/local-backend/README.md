# Local Backend Example

This runnable example uses the reusable Local Backend Composition and adds an
example-owned application Component. It writes local data to SQLite through the
portable `RelationalDatabase` Resource, reads it back, emits a log record, and
serves the stored data through one real loopback HTTP request.

Run from the repository root:

```sh
cargo run -p fabric-ecosystem-example-local-backend
```

Expected output includes:

```text
database value: local-data
log target: local-backend-example
request: GET /local-backend
response: 200 local-data via /local-backend
```

Uses:

- `fabric-composition-local-backend`
- `fabric-package-relational-database`
- `fabric-package-observability-logging`
- `fabric-package-networking-http`
- `fabric-package-networking-tcp`

The SQLite file is created under the system temporary directory with a unique
name and cleaned up by the example. The application behavior and error boundary
belong to this example; they are not reusable package semantics.

Source map:

- `src/main.rs`: Local Backend orchestration, HTTP exchange, cleanup, and output.
- `src/app.rs`: example-owned database/logging Component and error boundary.
- `src/client.rs`: local loopback HTTP client used by the demonstration.

This example does not demonstrate routing, authentication, migrations,
connection pooling, durable service management, or a web framework.
