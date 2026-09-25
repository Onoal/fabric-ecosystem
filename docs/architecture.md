# Fabric Ecosystem Architecture

Fabric Ecosystem is the source home for shareable artifacts built with Fabric:
Packages, Compositions, and Examples.

```text
FABRIC CORE
    structural construction machinery
        |
        v
FABRIC
    semantic systems-construction world
        |
        v
PACKAGES
    reusable building material
        |
        v
COMPOSITIONS
    reusable assembled systems

EXAMPLES
    demonstrate any or all layers
```

These repository families are source ownership categories. They are not new
Fabric semantic primitives.

## Artifact Law

- Package = reusable building material.
- Composition artifact = reusable assembled system.
- Example = teaching or demonstration artifact.

Package != Composition. Composition != Example. Example != Package.

None of these repository artifact kinds introduces `PackageId`,
`CompositionArtifactId`, `ExampleId`, `EcosystemArtifact`, or
`EcosystemRuntime`.

## Packages

Fabric packages are reusable Rust crates that publish ordinary Fabric
definitions and contribution helpers.

They are not Fabric semantic truth. A package may define resources, systems,
components, adapters, augmentations, and `FabricContribution` helpers, but after
authoring the resulting `Composition` contains only Fabric semantic truth:
resources, systems, components, relations, realizations, augmentations, and
live instance observations.

### Topology

Packages are grouped by capability ownership:

- `packages/data/key-value` owns the KeyValue capability.
- `packages/execution/process-runtime` owns local process execution.
- `packages/messaging/queue` owns the first messaging capability: a bounded
  FIFO queue.
- `packages/networking/tcp` owns the first network/transport capability:
  loopback TCP byte streams.
- `packages/observability/counter` owns the first observability capability:
  monotonic counter metrics.
- `packages/security/ed25519` owns the first security capability: concrete
  Ed25519 signing.

This repository deliberately avoids kind-based folders such as `resources/` or
`adapters/`. Those folders would make implementation machinery look more
important than capability ownership.

### Reserved Capability Pressure

The foundation is intentionally breadth-first, not exhaustive.

- Network and transport packages should own real connectivity capabilities,
  not merely wrap process execution or key-value state.
- Future security packages should expose concrete cryptographic or
  authorization capabilities without borrowing identity concepts from other
  Onoal systems.
- Future observability packages may own logs, traces, exporters, or audit
  behavior when those capabilities are implemented as real Fabric definitions
  and contributions.

None of these reserved families should appear as empty crates. A package family
is introduced only when it contains useful Fabric authoring.

### Messaging Queue

`packages/messaging/queue` establishes the first real messaging capability. The
semantic definition is `FifoQueue`, a named Resource occurrence representing one
queue. No System is required for the first local model because there is no
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

### TCP Byte-Stream Transport

`packages/networking/tcp` establishes the first real external I/O capability.
The semantic definition is `TcpByteStreamTransport`, a named Resource
occurrence representing one TCP byte-stream listener/connector capability. TCP
is the semantic contract for this first package; the local loopback
implementation is the Adapter realization.

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

### Ed25519 Signing

`packages/security/ed25519` establishes the first real cryptographic
capability. The semantic definition is `Ed25519Signer`, a named Resource
occurrence representing one live signing capability. The package is
Ed25519-specific on purpose; a generic signing abstraction is premature until
more algorithms and key-management models have been pressure-tested.

The model is intentionally small and concrete:

- Messages are opaque bytes.
- `sign` returns an Ed25519 signature over the exact bytes supplied.
- `public_key` returns the live verifying key for the current generation.
- Verification is a pure package function over public key, message bytes, and
  signature bytes. It is not a Fabric Resource, System, Component, authority
  lookup, trust decision, or certificate validation step.
- The private key is generated during materialization/start and belongs to the
  Adapter's live runtime state.
- The private key is not Composition truth, not Instance observation truth, and
  not printed by package value `Debug` implementations.
- The first adapter is ephemeral and in-memory. Stopping a generation drops its
  live key state; a fresh generation receives fresh live key state.
- Durable key stores, hardware-backed signing, HSMs, remote signers, key
  rotation, certificate chains, and policy are future adapter or package
  pressure, not part of this first capability.

An Ed25519 public key is public evidence for signature verification. It is not
an identity, authority, account, owner, certificate, DID, Origin principal, or
Identis subject. A valid signature proves only that the corresponding private
key signed the bytes. It does not prove who owns the key or whether the result
should be trusted.

The package includes `SignedPayloadProducer` as a Component witness: behavior
may consume the signer Resource through a normal Fabric relation and publish a
payload plus public evidence. The signer Resource owns the cryptographic
capability; the Component owns the application behavior that decides what bytes
to sign.

### Counter Metrics

`packages/observability/counter` establishes the first real observability
capability. The semantic definition is `CounterMetric`, a named Resource
occurrence representing one monotonic counter. Occurrence names such as
`requests`, `successful-sends`, or `failed-sends` are the metric identity for
this first model; there is no separate metric registry, label system, or
`MetricId`.

The model is intentionally small:

- A counter starts at zero for a fresh in-memory generation.
- `increment(amount)` adds a non-negative `u64` amount.
- `current()` reads the current aggregate value.
- There is no decrement, arbitrary set, or hidden reset during a running
  generation.
- Overflow is explicit: an increment that would exceed `u64::MAX` returns
  `CounterIncrementResult::Overflow` and leaves the current value unchanged.
- Counter values are live Adapter/runtime state. They are not Composition
  truth and do not appear before materialization/start.

Fabric observation and application observability remain separate. Fabric
`Instance::observe()` answers generic system questions such as lifecycle,
realization, participation, and local health. Counter metrics answer
package/application telemetry questions such as how many successful sends were
observed by a behavior. Metrics are accessed through the package semantic API,
not through a generic Fabric telemetry registry.

Counter values are not health. A `failed-sends` value of `10` is telemetry; it
does not automatically make a Component or Adapter unhealthy. Conversely, a
healthy metric Adapter says only that the metric machinery is available, not
that the observed application is good.

Counter values are aggregate telemetry, not history or audit evidence. A value
of `1842` does not contain 1842 event records, timestamps, actors, payloads,
causal relationships, retention policy, or authoritative proof. Logs, traces,
durable audit, and identity-aware telemetry remain separate future pressure.

Instrumentation is explicit application behavior. An application Component may
require both `FifoQueue` and `CounterMetric`, perform a queue send, and then
increment success or failure counters according to the actual `QueueSendResult`.
The queue package does not depend on observability, and the observability
package does not depend on queue semantics.

Future adapters may export metrics to a Prometheus-style collector,
OpenTelemetry pipeline, or remote metrics service. That is realization concern
or future package pressure. The counter semantic contract does not require
scrape or push, and Fabric does not claim ownership of remote collector state.

Metrics do not record Ed25519 private keys, arbitrary TCP bytes, queue payloads,
or other sensitive package internals by default. A counter records only the
explicit measured fact selected by the behavior that increments it.

This package deliberately does not introduce OXP `Observation`, Oracle
execution evidence, Origin/Identis identities, tracing spans, logging records,
or audit authority.

## Compositions

`compositions/` is for shareable source artifacts that assemble coherent Fabric
systems. Fabric already has `fabric::Composition`; the repository family means
source that constructs ordinary Fabric Composition truth.

A reusable Composition artifact should represent something with assembly value:
a complete HTTP server, storage server, compute server, worker system, local
development stack, or another coherent assembled system.

A Composition may consume Packages, but Composition != bundle of Packages. It
may contain package-provided definitions, project-specific definitions, custom
Components, custom config, explicit relations, and ordinary Fabric authoring.
Its defining property is coherent assembled system truth.

Illustrative future classification:

- `packages/networking/tcp`: reusable TCP capability.
- `packages/networking/http`: possible future reusable HTTP protocol
  capability if earned.
- `compositions/servers/http-server`: possible future complete HTTP-serving
  system.
- `packages/data/...`: reusable storage/data capabilities.
- `compositions/servers/storage-server`: possible future assembled
  storage-serving system.
- `packages/execution/...`: reusable execution/compute capabilities.
- `compositions/servers/compute-server`: possible future assembled compute
  serving system.

These are examples, not implemented artifacts. A `servers/` taxonomy would be a
repository taxonomy, not a Fabric `Server` semantic primitive.

## Examples

`examples/` contains pedagogical artifacts. An Example may demonstrate raw
Fabric, one Package, multiple Packages, a reusable Composition, lifecycle, or
Instance behavior. Example ownership is teaching ownership, not semantic
ownership.

Compatibility and integration fixtures remain under `tests/`.

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
