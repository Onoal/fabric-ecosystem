# Linux Host

`onoal-fabric-host-linux` is the first real Host artifact in Fabric Ecosystem.

It owns reusable Linux environment detection, stable Linux facility identifiers,
and Host requirement helpers built on Fabric's existing Host model.

```text
Host fact
!= semantic capability
!= realization
!= policy
```

Fabric owns `HostDescriptor` and `HostRequirement`. This crate returns those
canonical Fabric values directly. It does not define a `LinuxHostDescriptor`
and does not introduce a Linux Resource, System, Component, or Adapter.

## Detection

`detect_linux_host()` returns a Fabric `HostDescriptor` on Linux and a bounded
`LinuxHostError::UnsupportedPlatform` on non-Linux targets.

The descriptor uses:

- operating system: `linux`
- architecture: Rust's native `std::env::consts::ARCH`, stored as Fabric
  `HostArchitecture`
- facilities: only evidence-backed facts detected at runtime

## Facilities

Current facility identifiers are:

- `linux.procfs`
- `linux.cgroup-v2`

Facilities are evidence-based. `linux.procfs` is advertised only when procfs
evidence is available. `linux.cgroup-v2` is advertised only when cgroup v2
evidence is available. Linux OS identity alone does not imply every Linux
facility.

Public helper functions expose the stable IDs so future ecosystem code can use
Fabric's open `HostFacilityId` model without copying string literals.

## Requirements

The crate provides reusable requirements:

- `linux_requirement()`
- `linux_procfs_requirement()`
- `linux_cgroup_v2_requirement()`

A future Adapter may use these to declare environment compatibility. A
requirement says what environment is compatible; it does not choose an Adapter
or encode realization policy.

## Boundaries

Docker is not a Host. Docker installation or availability is not detected here.
A future execution/runtime capability may use Docker as realization machinery,
but that is separate from Linux Host facts.

This crate deliberately does not detect or model Podman, systemd, GPUs, CPU
topology, RAM, disks, network interfaces, cloud metadata, hostname identity,
machine identity, remote hosts, schedulers, placement, Oracle concepts, or a
Host registry.
