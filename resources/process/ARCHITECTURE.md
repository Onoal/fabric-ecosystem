# Fabric Process Resource

Process is a re-founded candidate Fabric Resource.

A Process Resource represents preparation and managed execution of a native
executable process through explicit lifecycle and explicit consumption inputs.

Current canonical shape:

```text
ProcessContract
    ↓
Process Resource
    ↓
ProcessAdapter
    ↓
candidate concrete adapter
```

Process means:

- semantic native executable lifecycle
- explicit artifact + entrypoint preparation
- explicit start/stop semantics
- explicit projected process environment input
- stable Process instance identity independent of adapter machinery

Process does not mean:

- sandboxed Worker
- JavaScript runtime
- HTTP handler
- Service
- Host process registry
- systemd

Frozen FX4.1 law:

- Process Resource != Systemd Adapter
- Process Resource != Worker semantics
- Process Resource != Server semantics
- Process instance identity != systemd unit name
- Process environment input != Binding identity
- Binding != Projection

The current systemd implementation is only a candidate adapter. Final Standard
qualification is deferred to FX5 freeze.
