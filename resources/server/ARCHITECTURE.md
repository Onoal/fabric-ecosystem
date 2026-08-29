# Fabric Server Resource

Server is a candidate Fabric execution Resource for preparation and managed
lifecycle of a long-lived self-serving workload.

Execution remains DX grouping only:

- Worker: Fabric-driven invocation
- Server: self-serving lifecycle
- Process: native executable lifecycle

Server means:

- semantic server artifact and entrypoint preparation
- managed start and stop lifecycle
- stable Server instance identity independent of adapter machinery
- optional serving facets such as HTTP

Server does not mean:

- Service identity
- Ingress
- Gateway
- Worker invocation
- native Process lifecycle
- Deno
- public reachability

Canonical boundary:

```text
ServerContract
    ↓
optional ServerHttpContract
    ↓
ServerAdapter
    ↓
candidate concrete adapter
```

Frozen FX4.4 laws:

- Server Resource != Worker Resource
- Server Resource != Process Resource
- Server Resource != Service
- Server Resource != Ingress
- Server Resource != Gateway
- Server instance identity != adapter transport identity
- optional Server HTTP facet != Server definition

The current Deno realization is only a candidate adapter. Its binary path,
runtime root, loopback endpoint, host/port launch convention, and readiness
probing remain adapter-private machinery. The user Server entrypoint itself
owns the serving lifecycle.

Service remains separate. A running HTTP-capable Server may become a Service
target through an external bridge, while `ServiceId` remains stable across
Server instance replacement.

Final Standard qualification is deferred to FX5 freeze.
