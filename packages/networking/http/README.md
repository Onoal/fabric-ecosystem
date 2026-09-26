# Fabric Package: Networking HTTP

`onoal-fabric-package-networking-http` owns bounded HTTP/1 server behavior over
the existing TCP byte-stream capability.

```text
TCP bytes
!=
HTTP request/response behavior
```

`TcpByteStreamTransport` remains the transport Resource. `HttpServer` is a
behavioral Component that requires a named TCP transport occurrence and performs
one HTTP exchange:

```text
accept one TCP connection
parse one HTTP/1 request
write one HTTP/1 response
close the connection
return the parsed request
```

This first server behavior supports HTTP/1.0 and HTTP/1.1 request parsing,
`Content-Length` request bodies, package-owned request/response/header types,
and explicit request-size bounds. It does not claim keep-alive, routing,
middleware, TLS, HTTP/2, HTTP/3, WebSockets, SSE, proxying, authentication,
authorization, static files, JSON semantics, database access, or logging
integration.

The request-head bound is `DEFAULT_MAX_HEAD_BYTES`. The request-body bound is
`DEFAULT_MAX_BODY_BYTES`. Chunked transfer decoding is intentionally deferred.

HTTP errors are package-owned. Parser-library types, TCP implementation error
types, `std::io::Error`, and runtime socket values are not part of the public
HTTP semantic contract.

Typical local authoring:

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

Future packages or compositions may layer routing, logging, persistence, TLS, or
application behavior on top. This package deliberately establishes only the
protocol behavior boundary.
