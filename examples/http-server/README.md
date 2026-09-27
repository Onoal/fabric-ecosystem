# HTTP Server Example

This runnable example uses the reusable HTTP Server Composition, materializes it
on loopback with an ephemeral TCP port, discovers the actual bound address,
serves one real HTTP/1 request, and stops.

Run from the repository root:

```sh
cargo run -p fabric-ecosystem-example-http-server
```

Expected output includes:

```text
HTTP server bound to 127.0.0.1:...
request: GET /example
response: 200 hello from /example
```

Uses:

- `fabric-composition-http-server`
- `fabric-package-networking-http`
- `fabric-package-networking-tcp`

The local client exists only to make `cargo run` self-contained. This example
does not demonstrate routing, a web framework, TLS, async runtimes, or
long-running server supervision.

Source map:

- `src/main.rs`: Fabric composition, materialization, exchange, and output.
- `src/client.rs`: local loopback HTTP client used by the demonstration.
