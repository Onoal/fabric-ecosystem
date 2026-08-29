# Fabric Deno Server Adapter

`DenoServerAdapter` is the first candidate adapter for the Server Resource.

It realizes Server semantics with Deno machinery while keeping Deno private to
the adapter boundary.

The adapter owns:

- `DENO_BIN`
- runtime root layout
- artifact staging
- loopback endpoint allocation
- adapter-private host/port environment convention
- readiness probing
- Deno process spawning and cleanup

`DenoServerAdapter` launches the requested staged Server entrypoint directly.
The user program itself establishes and owns the long-lived serving lifecycle,
for example by calling `Deno.serve(...)`.

This differs intentionally from Worker realization:

- `DenoWorkerAdapter` may wrap and invoke a workload according to Worker semantics
- `DenoServerAdapter` launches a program that owns its own serving lifecycle

The adapter does not define Server semantics. It is only one candidate
realization of:

- `ServerContract`
- optional `ServerHttpContract`

The local loopback listener is adapter-private serving transport. It is not:

- `ServerId`
- `ServerInstanceId`
- `ServiceId`
- public reachability

Readiness is transport-level only. The adapter waits for successful TCP
acceptance on its private loopback endpoint and does not reserve any user HTTP
route such as `/__fabric_ready`.

Future Server adapters may include Node or native-binary realizations without
changing the Server Resource contract.
