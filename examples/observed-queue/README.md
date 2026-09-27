# Observed Queue Example

This runnable example shows application-owned instrumentation over a bounded
queue. It creates a capacity-one in-memory FIFO queue and two in-memory
counters. The first send succeeds and increments the success counter. The second
send observes full capacity and increments the failure counter.

Run from the repository root:

```sh
cargo run -p fabric-ecosystem-example-observed-queue
```

Expected output includes:

```text
first send: accepted
second send: full at capacity 1
successful sends: 1
failed sends: 1
```

Uses:

- `fabric-package-messaging-queue`
- `fabric-package-observability-counter`

Queue semantics and Counter semantics remain separate. The example-owned
Component decides what to count and when.

Source map:

- `src/main.rs`: scenario orchestration and visible metrics.
- `src/app.rs`: example-owned Component requiring the queue and counters.
