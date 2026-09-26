# TCP Byte-Stream Transport

`onoal-fabric-package-networking-tcp` owns a bounded TCP byte-stream transport
capability for Fabric.

## Purpose

`TcpByteStreamTransport` represents one named TCP listener/connector
capability. It is byte-oriented transport, not an application protocol.

```text
TCP bytes
!= HTTP
!= OXP Endpoint/Lane/Session
!= application framing
```

## Architecture

- `TcpByteStreamTransport`: semantic Resource for bind/listen, connect, accept,
  and runtime listener facts.
- `TcpConnection`: live runtime value for one accepted or connected stream.
- `LoopbackTcpByteStream`: first OS TCP realization, constrained to loopback
  bind addresses.
- `TcpTransportInspector`: reusable Component for runtime inspection, including
  the actual bound address of an ephemeral listener.

Transport operations remain on `TcpByteStreamTransport`. `TcpTransportInspector`
only observes runtime facts; it does not connect, accept, read, or write.

## Authoring

```rust
use fabric::prelude::*;
use fabric_package_networking_tcp::{loopback_tcp_transport, tcp_transport_inspector};

let composition = Fabric::new("example")?
    .with(loopback_tcp_transport("api"))
    .with(tcp_transport_inspector("api"))
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `loopback_tcp_transport_on(name, address)` when the bind address should be
explicit. The loopback realization accepts only loopback bind hosts such as
`127.0.0.1`; binding `0.0.0.0` belongs to a future non-loopback realization or
renamed boundary.

## Connection Semantics

`TcpConnection` is a runtime value associated with the current materialized
generation. It is not a Fabric Resource, Component, System, semantic endpoint
identity, or OXP Session.

- `write_all(bytes)` writes the whole buffer or returns a package-owned error.
- `read_some(max_bytes)` performs one underlying blocking stream read. It may
  block until bytes, EOF, or error. `max_bytes == 0` returns an empty buffer
  immediately.
- `read_to_end()` reads until peer EOF.
- `shutdown_write()` half-closes the write side.
- `shutdown_both()` closes both directions.

After the owning generation stops, stale connection operations fail with
`Stopped` rather than pretending to be current live transport.

## Accept Semantics

`accept()` waits for the next queued accepted connection while the realization
is live. Stopping the Instance unblocks waiting accept calls and returns
`Stopped`. Timeout and nonblocking accept are deliberate future extensions, not
part of this first v1 TCP foundation.

## Runtime Address Discovery

When binding to `127.0.0.1:0`, the OS selects the actual port at start time.
That address is live runtime truth. Use `TcpTransportInspector` to inspect it;
do not put live ports into Composition truth. Connection establishment remains
owned by `TcpByteStreamTransport::connect(...)`.

## Multiple Occurrences

Multiple named `TcpByteStreamTransport` Resource occurrences may coexist. They
own independent listener state, accepted connection counts, and bound ports per
Instance generation.

## Host and Platform Stance

The first realization uses Rust standard-library OS TCP on the local machine.
It does not model remote host identity, remote health, internet availability,
Docker, DNS, TLS, or public exposure policy.

## Extension Path

Future realizations or packages may add non-loopback binding, IPv6 policy,
timeouts, socket options, TLS, DNS, proxying, or async runtime integration. They
should not change the byte-stream ownership of this package.

## Non-goals

- TLS
- DNS
- HTTP
- OXP Endpoint, Lane, Session, or Exchange
- application framing
- routing
- proxying
- connection pooling
- distributed transport
