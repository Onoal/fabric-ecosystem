# Fabric Package Architecture

Fabric packages are reusable Rust crates that publish ordinary Fabric
definitions and contribution helpers.

They are not Fabric semantic truth. A package may define resources, systems,
components, adapters, augmentations, and `FabricContribution` helpers, but after
authoring the resulting `Composition` contains only Fabric semantic truth:
resources, systems, components, relations, realizations, augmentations, and
live instance observations.

## Topology

Packages are grouped by capability ownership:

- `data/key-value` owns the KeyValue capability.
- `execution/process-runtime` owns local process execution.
- `messaging/queue` owns the first messaging capability: a bounded FIFO queue.
- `networking/tcp` owns the first network/transport capability: loopback TCP
  byte streams.

This repository deliberately avoids kind-based folders such as `resources/` or
`adapters/`. Those folders would make implementation machinery look more
important than capability ownership.

## Reserved Capability Pressure

The foundation is intentionally breadth-first, not exhaustive.

- Network and transport packages should own real connectivity capabilities,
  not merely wrap process execution or key-value state.
- Security and crypto packages should expose concrete cryptographic or
  authorization capabilities without borrowing identity concepts from other
  Onoal systems.
- Observability packages should own telemetry, metrics, traces, or audit
  behavior when those capabilities are implemented as real Fabric definitions
  and contributions.

None of these reserved families should appear as empty crates. A package family
is introduced only when it contains useful Fabric authoring.

## Messaging Queue

`messaging/queue` establishes the first real messaging capability. The semantic
definition is `FifoQueue`, a named Resource occurrence representing one queue.
No System is required for the first local model because there is no
instance-wide broker/environment truth beyond the named queue occurrence.

Delivery semantics are intentionally small and explicit:

- Payloads are opaque bytes.
- Messages have no first-class `MessageId`.
- `send` appends to the tail when capacity remains.
- `try_receive` is non-blocking, destructive, and removes from the head.
- Ordering is FIFO for accepted messages.
- Multiple consumers compete for messages; this is not fan-out.
- Capacity is in-memory adapter configuration for this first realization.
- Empty receive returns `None`.
- Full queue returns `QueueSendResult::Full`.
- The in-memory realization is not durable, recoverable, replayable, or
  acknowledged.

Future adapters may realize the same semantic queue through remote systems such
as NATS, Redis Streams, Kafka, AMQP, or an OXP-backed transport, but Fabric does
not become owner of remote canonical state. OXP remains transport/exchange
machinery, Oracle remains planning/control, and Origin/Identis identity or
authority concepts are not mandatory messaging primitives.

## TCP Byte-Stream Transport

`networking/tcp` establishes the first real external I/O capability. The
semantic definition is `TcpByteStreamTransport`, a named Resource occurrence
representing one TCP byte-stream listener/connector capability. TCP is the
semantic contract for this first package; the local loopback implementation is
the Adapter realization.

The model is intentionally technical and bounded:

- It uses explicit socket addresses, normally `127.0.0.1:0` in tests.
- The requested bind address is adapter configuration.
- The actual bound address and port are live runtime truth, available only
  after materialization/start through the package API.
- Connections are runtime values, not Fabric Resources,
  Components, Systems, or identities.
- The contract is byte-oriented. It does not define messages, frames, RPC,
  HTTP, JSON, or FIFO delivery.
- The loopback adapter owns socket bind/listen/connect/accept mechanics and
  accepted-connection delivery. It does not own echo or request/response
  behavior.
- A connection exposes bounded read, write, and shutdown operations as live
  runtime value behavior.
- Behavioral Components or applications may implement echo on top of the
  transport by accepting a connection, reading bytes, and writing bytes.
- The adapter owns a background accept loop internally and releases it during
  synchronous Fabric `stop`.
- Health describes local transport machinery, not remote peer health.

This package deliberately does not create OXP concepts such as Endpoint, Lane,
Locator, Path, Session, Exchange, Policy, Observation, or Protocol. Its public
TCP model avoids `Exchange` vocabulary. A future OXP adapter may use this kind
of transport, but transport bytes are not OXP exchange semantics. TLS,
certificates, DNS, hostnames, URI/Locator models, and remote identity/authority
remain future separate capability pressure.

## Fabric Boundary

Fabric 0.7 remains unchanged. Package helpers use public Fabric APIs:

- `resource!`
- `system!`
- `component!`
- `adapter!`
- `FabricContribution`
- `Composition` inspection
- `Instance` observation and typed invocation

No package introduces `Package`, `PackageId`, `PackageManifest`, or package
runtime state into Fabric itself.
