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

Messaging currently has no Composition artifact. The previous local queue
pipeline was removed because it wrapped one Resource with proxy Components
rather than adding a meaningful reusable assembly opinion.

Good future candidates include storage-serving systems, compute-serving
systems, worker systems, or additional local development stacks.

Do not add a Composition artifact merely to bundle all current packages
together. A Composition may use ecosystem packages, project-specific Fabric
definitions, custom Components, custom config, and explicit relations. It is not
defined by package count.

A Composition must add a meaningful reusable assembly opinion. Wrapping one
Resource, or adding Components that only proxy that Resource, does not earn a
Composition artifact.

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
