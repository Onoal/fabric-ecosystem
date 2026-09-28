# Fabric Package: Messaging Queue

`onoal-fabric-package-messaging-queue` owns one deliberately small messaging
capability: a bounded, non-durable, non-blocking FIFO queue.

## Purpose

`FifoQueue` represents one named queue occurrence. Producers, consumers, and
workers are application-owned Components that require `FifoQueue` and call its
Resource API directly.

The package does not provide generic proxy Components. A Component exists only
when it owns real reusable behavior beyond forwarding queue methods.

## Semantic Ownership

- `FifoQueue`: Resource for FIFO send, destructive non-blocking receive, and
  depth observation.
- `QueueMessage`: opaque byte payload.
- `QueueSendResult`: `Accepted` or `Full { capacity }`.
- `InMemoryQueue`: generation-local in-memory realization.
- `memory_queue(name, capacity)`: authoring helper for a named in-memory queue.

## Capacity

`InMemoryQueue` capacity must be greater than zero. `memory_queue(...)` returns
`QueueConfigError::ZeroCapacity` for invalid configuration.

When the queue is full, `send(payload)` returns `QueueSendResult::Full` and the
payload is not enqueued.

## FIFO And Receive Semantics

Messages are opaque bytes and are returned in FIFO order.

`try_receive()` is non-blocking and destructive:

```text
receive returns Some(message)
    -> message is removed

receive returns None
    -> queue was empty
```

There is no ack, nack, redelivery, visibility timeout, persistence, recovery,
or durable broker behavior.

Queue Resource operations are awaitable like all Fabric Resource operations:

```rust
let sent = jobs.send(b"job".to_vec()).await;
let message = jobs.try_receive().await;
let depth = jobs.depth().await;
```

## Lifecycle

`InMemoryQueue` state is live Instance generation state. A fresh materialization
starts with an empty queue, even with the same Composition.

## Multiple Queues And Consumers

Multiple named queue occurrences may coexist:

```text
memory_queue("jobs", 64)
memory_queue("events", 64)
```

Multiple consumer-owned Components requiring the same `FifoQueue` compete by
shared destructive receive. No special `CompetingConsumer` Component is needed.

## Consumer-Owned Component Shape

Consumer-owned Components should require `FifoQueue` directly and own their
application behavior. Current Fabric Component runtime methods are synchronous,
so local examples bridge immediate Resource operations at that boundary. The
Resource contract itself remains awaitable.

## Extension Path

Future messaging packages may add durable brokers, acknowledgements, retries,
dead-letter queues, visibility timeouts, scheduled delivery, priority queues,
or worker runtimes when those semantics are earned.

## Non-goals

- durable broker
- ack/nack
- retries
- dead-letter queue
- visibility timeout
- scheduled delivery
- priority queue
- Kafka, NATS, RabbitMQ, or Redis semantics
- WorkerRuntime
- scheduler
