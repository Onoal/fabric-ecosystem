# Local Queue Pipeline Composition

`onoal-fabric-composition-local-queue-pipeline` assembles one local messaging
pipeline from the Queue package:

- `FifoQueue("<name>")` realized by `InMemoryQueue`;
- `QueueProducer` bound to the queue as `outbox`;
- `QueueConsumer` bound to the same queue as `inbox`.

This is a Composition, not a Package. The Queue package owns the reusable
messaging semantics. This crate owns the assembly opinion: one bounded
in-memory FIFO queue with one producer and one consumer.

The queue name and capacity are explicit configuration. Capacity must be
greater than zero. The in-memory realization is non-durable: messages are live
generation-local state and a fresh Fabric generation starts empty.

Application or worker behavior remains consumer-owned. A larger system can add
its own Component requiring `FifoQueue` and process messages however it wants.
The existing Queue package also exposes `CompetingQueueConsumer`; it can be
added explicitly by a consumer when destructive competing consumption is wanted,
but it is not part of the default pipeline.

Current Fabric v1 Component identity is definition-scoped. Because this
Composition includes the package's `QueueProducer` and `QueueConsumer`
definitions, one default Local Queue Pipeline component pair is expected per
Fabric Composition. Multiple independent `FifoQueue` Resource occurrences are
still valid; the limitation is Component definition identity, not queue
Resource multiplicity.

Non-goals:

- WorkerRuntime;
- Deno or process execution;
- durable brokers;
- retries, acknowledgements, visibility timeouts, or dead-letter queues;
- schedulers or background daemons;
- logging, database, HTTP, or deployment integration.
