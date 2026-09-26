# Fabric Composition: HTTP Server

`onoal-fabric-composition-http-server` is the first reusable Composition
artifact in Fabric Ecosystem. It assembles an ordinary local HTTP-serving
system from existing packages:

```text
TcpByteStreamTransport
    realized by LoopbackTcpByteStream
        |
        +-- TcpTransportInspector
        `-- HttpServer
```

This is a Composition artifact, not a Package. The Packages own the reusable
capabilities:

```text
packages/networking/tcp
    TCP byte-stream transport capability

packages/networking/http
    HTTP/1 request/response behavior over TCP

compositions/web/http-server
    opinionated local assembly of TCP realization, runtime address discovery,
    and HTTP server behavior
```

The local default binds loopback `127.0.0.1:0`. The actual bound address is
runtime truth and remains discoverable through `TcpTransportInspector`; this
Composition does not invent a second address-inspection mechanism.

Fabric v1 Component identity is definition-scoped, so this first assembly
supports one `HttpServer` Component definition occurrence per built
`fabric::Composition`. The TCP occurrence name is still explicit, and the stack
can be reused inside larger systems.

Use `http_server_stack(config)` to contribute the assembly to a larger Fabric
authoring context, or `build_http_server_composition(id, config)` to build a
standalone ordinary `fabric::Composition`. Both paths use the same assembly.

Non-goals: routing, application handlers, database access, key-value storage,
logging, metrics, TLS, public internet exposure, reverse proxying, worker
runtime, deployment, scheduling, CLI, registry, or STEL integration.
