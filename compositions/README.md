# Compositions

This family contains reusable assembled Fabric systems.

A Composition artifact is source that constructs or configures a coherent
`fabric::Composition`. Its value is the assembly itself, not a single reusable
capability.

`compositions/web/http-server` is the first real artifact. It assembles the TCP
package's local loopback realization, TCP runtime address discovery, and the
HTTP package's one-request server behavior into one reusable local HTTP-serving
system.

`compositions/web/local-backend` is the first nested Composition. It reuses the
HTTP Server assembly and adds SQLite relational persistence plus Console
logging as a local backend foundation. Application behavior remains ordinary
consumer-owned Fabric Components.

`compositions/messaging/local-queue-pipeline` is the first messaging
Composition family. It assembles one named in-memory `FifoQueue`, one
`QueueProducer`, and one `QueueConsumer`. Worker behavior remains
consumer-owned; this Composition is not a WorkerRuntime, scheduler, or durable
broker.

The web and messaging Compositions both record the current Fabric v1 Component
identity law: reusable assemblies can preserve explicit named Resource
occurrences, while repeated use of the same package Component definitions in
one built Composition is constrained by definition-scoped Component identity.

Good future candidates include storage-serving systems, compute-serving
systems, worker systems, or additional local development stacks.

Do not add a Composition artifact merely to bundle all current packages
together. A Composition may use ecosystem packages, project-specific Fabric
definitions, custom Components, custom config, and explicit relations. It is not
defined by package count.

The expected shape is:

```text
compositions/<category>/<composition>/
├── Cargo.toml
├── README.md
├── src/
│   └── lib.rs
├── tests/      # when useful
└── variants/   # only when real variants exist
```

A variant is an ecosystem authoring opinion, not a Fabric primitive. Do not
create `variants/` before real variants exist.
