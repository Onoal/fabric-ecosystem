# Fabric Systemd Process Adapter

`fabric-adapter-process-systemd` is a candidate concrete adapter for the
Process Resource.

It is not:

- the Process Resource itself
- the canonical `ProcessContract`
- Resource Registry participation
- a generic host/runtime substrate

The canonical boundary is:

```text
NativeProcess
    ↓
ProcessContract
    ↓
ProcessAdapter
    ↓
SystemdProcessAdapter
```

The Systemd adapter owns only concrete machinery:

- systemd unit construction and supervision
- artifact staging and digest verification support
- provider-private prepared runtime state
- transient runtime directories
- Linux/systemd host compatibility requirements

The Process Resource still owns:

- `ResourceId("process")`
- `ProcessSpec`
- `PreparedProcess`
- semantic prepare/start/stop truth
- process instance identity
- explicit projected environment semantics

Frozen FX4.1 laws:

- Process Resource != Systemd Adapter
- Process Resource != Host
- Process instance identity != PID
- Process instance identity != systemd unit name
- Process environment input != Binding identity
- Binding != Projection

Current projection behavior remains intentionally narrow. The Systemd adapter
accepts only public environment material and rejects projection forms it cannot
honestly materialize, including:

- structured projections
- sensitive environment projections
- network-authority projections

Systemd-specific mechanics remain private implementation detail. The Process
Resource API contains no systemd vocabulary.

FX3 Host compatibility remains explicit for this adapter:

- operating system: `linux`
- required facility: `fabric.host.user-systemd-supervision`

That Host requirement is evaluated before supervisor construction. Production
construction therefore requires an explicitly enriched `HostDescriptor`;
`HostDescriptor::native()` remains OS/architecture-only. Runtime cfg checks
remain defense-in-depth, not the only compatibility declaration.
