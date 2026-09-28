# Fabric Ecosystem Instances

`instances/` contains runnable operational entrypoints.

An Instance artifact is source code that materializes and operates a live
`fabric::Instance` occurrence. The source artifact itself is not live runtime
truth, persisted Instance state, scheduler state, or deployment state.

```text
Composition
    reusable assembly
        |
        v
Instance artifact
    operational entrypoint
        |
        v
materialized fabric::Instance
    live runtime occurrence
```

Instances are not Examples. Examples are bounded teaching applications that
demonstrate a scenario and exit. Instance artifacts run foreground operational
processes and remain alive until the operator terminates them.

Instances are not Compositions. A Composition owns reusable assembly meaning.
An Instance artifact selects one operational entrypoint for running that
assembly.

Current Instance artifacts:

- `web/http-server`: long-running loopback HTTP server.
