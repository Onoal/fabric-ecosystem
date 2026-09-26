# Compositions

This family contains reusable assembled Fabric systems.

A Composition artifact is source that constructs or configures a coherent
`fabric::Composition`. Its value is the assembly itself, not a single reusable
capability.

`compositions/web/http-server` is the first real artifact. It assembles the TCP
package's local loopback realization, TCP runtime address discovery, and the
HTTP package's one-request server behavior into one reusable local HTTP-serving
system.

It also records the current Fabric v1 Component identity law: the first
assembly provides one `HttpServer` Component definition occurrence per built
Composition, while preserving explicit named TCP occurrence selection.

Good future candidates include storage-serving systems, compute-serving
systems, worker systems, or local development stacks.

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
