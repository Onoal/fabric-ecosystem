# Compositions

This family is reserved for reusable assembled Fabric systems.

A Composition artifact is source that constructs or configures a coherent
`fabric::Composition`. Its value is the assembly itself, not a single reusable
capability.

Good future candidates include complete HTTP-serving systems, storage-serving
systems, compute-serving systems, worker systems, or local development stacks.

Do not add a Composition artifact merely to bundle all current packages
together. A Composition may use ecosystem packages, project-specific Fabric
definitions, custom Components, custom config, and explicit relations. It is not
defined by package count.
