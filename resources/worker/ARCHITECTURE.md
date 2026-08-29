# Fabric Worker Resource

Worker is a candidate Fabric execution Resource for Fabric-driven workload
invocation.

Worker means:

- workload artifact and entrypoint preparation
- explicit binding and projection consumption
- runtime instance lifecycle
- Fabric-driven invocation
- runtime result and error handling

Worker does not mean:

- native Process execution
- self-owned Server lifecycle
- Service identity
- Ingress
- Deno
- HTTP listener identity
- Host identity

Canonical boundary:

```text
NativeWorker
    ↓
WorkerContract
    ↓
optional WorkerHttpContract
    ↓
workload projection specialization
    ↓
prepared consumer projection material
    ↓
WorkerAdapter
    ↓
candidate concrete adapter
```

Worker owns:

- stable `ResourceId("worker")`
- `WorkerContract`
- optional `WorkerHttpContract`
- `WorkerSpec`
- `PreparedWorker`
- `WorkerInstanceId`
- prepare / start / stop / dispatch truth
- workload-facing binding and projection specialization

Worker does not own provider machinery such as:

- runtime binaries
- runtime roots
- process spawning
- loopback listeners
- bridge URLs
- permission flags
- staged filesystem layout

Those remain adapter-private implementation detail.

Prepared workload truth is canonical Worker truth:

- `WorkerSpec` is the requested execution description
- `PreparedWorker` proves Worker accepted that exact spec for later start
- adapter preparation state is private implementation detail only

The current Deno realization is only a candidate Worker adapter. Its loopback
listener and bootstrap transport are adapter-private invocation machinery, not
Worker identity, not Server identity, and not Binding identity.

Frozen execution laws:

- Worker Resource != Deno Adapter
- Worker Resource != Process Resource
- Worker Resource != Server
- Worker Resource != Adapter
- Worker instance identity != adapter transport identity
- valid Worker adapter does not imply HTTP
- optional Worker HTTP facet does not imply Server semantics

Workload consumers depend on Worker, not Deno. They prepare Resource
dependencies and construct semantic `WorkloadBinding` values only. Worker
bindings carry:

- stable consumer identity via `WorkloadId`
- Resource Ref identity such as `DatabaseRef`, `KvRef`, `SecretRef`, or `ServiceId`
- no provider materialization truth

Projection stays separate. `WorkloadBindingProjection` requests structured or
environment presentation of an already-existing binding. The shared Projection
rail remains consumer-neutral, while Worker owns the current workload
specialization:

- `WorkloadProjectionContract`
- `NativeWorkloadProjection`
- `PreparedWorkloadProjections`

Current canonical proofs cover:

- zero-binding Worker
- real Deno-backed Worker execution
- explicit consumer-aware DB/KV/Secret/Service binding
- binding validity without projection
- deterministic projection validation and cleanup
- real projected DB and Service access without changing underlying binding truth
- provider-owned runtime/process cleanup

Server is now a separate peer Resource with its own optional serving facets.
Worker remains the Fabric-driven invocation Resource.
