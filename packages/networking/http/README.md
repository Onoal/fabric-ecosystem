# Fabric Package: Networking HTTP

`onoal-fabric-package-networking-http` owns bounded HTTP/1 request/response
message behavior over the existing TCP byte-stream capability.

```text
TCP bytes
!=
HTTP request/response behavior
```

## Purpose

`TcpByteStreamTransport` remains the transport Resource. `HttpServer` is a
behavioral Component that requires a named TCP transport occurrence and accepts
one HTTP exchange per invocation.

Applications choose responses after inspecting requests:

```rust
let exchange = server.accept_exchange().await??;
let request = exchange.request();
let response = choose_response(request);
exchange.respond(response)?;
```

`HttpServer` is not a router, framework, callback registry, application
container, or long-running server loop.

## Architecture

- model: `HttpVersion`, `HttpHeader`, `HttpRequest`, `HttpResponse`
- server Component: `HttpServer`
- live exchange: `HttpExchange`
- codec machinery: internal request decoding and response encoding
- authoring helper: `http_server("tcp-name")`

`HttpExchange` is one accepted HTTP/1 request/response exchange. It is an
ordinary runtime value, not a Fabric Resource, Component, or System. It does
not expose the underlying `TcpConnection`.

## Public Flow

```text
HttpServer::accept_exchange()
    accepts one TCP connection
    decodes one bounded HTTP/1 request
    returns HttpExchange

HttpExchange::request()
    exposes the parsed request before response selection

HttpExchange::respond(response)
    serializes one HTTP/1 response
    closes the exchange
```

`respond(self, response)` consumes the exchange, so one exchange commits at
most one response. Dropping an exchange without responding closes the
connection; it does not fabricate a `500` or any other application policy.

## Supported Protocol Subset

- HTTP/1.0 and HTTP/1.1 request parsing
- one request, one response, then close
- `Content-Length` request bodies
- package-owned request, response, header, and error types
- bounded request head and body sizes

The request-head bound is `DEFAULT_MAX_HEAD_BYTES`. The request-body bound is
`DEFAULT_MAX_BODY_BYTES`. These are v1 defaults, not universal HTTP policy.

Chunked transfer decoding is intentionally unsupported and returns a bounded
`UnsupportedTransferEncoding` error.

## Validation

Response serialization owns `Content-Length`; caller-provided `Content-Length`
headers do not override the actual body size.

Response header names and values are validated enough to prevent CR/LF header
injection. Header names are case-insensitive by HTTP semantics, but this
package does not implement a typed-header framework.

Conflicting request `Content-Length` headers are rejected. Duplicate matching
values are accepted.

## Authoring

```rust
use fabric::prelude::*;
use fabric_package_networking_http::http_server;
use fabric_package_networking_tcp::loopback_tcp_transport;

let composition = Fabric::new("example")?
    .with(loopback_tcp_transport("api"))
    .with(http_server("api"))
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Extension Path

Future packages or compositions may layer routing, logging, persistence, TLS,
handler abstractions, or application behavior on top. This package deliberately
establishes only the HTTP protocol exchange boundary.

## Non-goals

- routing
- middleware
- web framework
- handler callback registry
- TLS
- HTTP/2 or HTTP/3
- WebSocket or SSE
- reverse proxying
- authentication or authorization
- JSON framework
- application handlers
- thread pool
- async runtime
