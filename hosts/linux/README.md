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

`detect_linux_host()` returns the canonical Fabric `HostDescriptor` for this
artifact on Linux and a bounded `LinuxHostError::UnsupportedPlatform` on
non-Linux targets.

Consumers should inspect the returned descriptor, its facilities, or evaluate
the requirement helpers against it. The raw `/proc` and cgroup evidence probes
are detection machinery, not a second public Host-truth API.

The descriptor uses:

- operating system: `linux`
- architecture: Rust's native `std::env::consts::ARCH`, projected into Fabric
  `HostArchitecture`
- facilities: only evidence-backed facts detected at runtime

Detection is observational. It does not mount filesystems, create cgroups,
change permissions, start services, or mutate host configuration.

## Facilities

Current facility identifiers are:

- `linux.procfs`
- `linux.cgroup-v2`

Facilities are evidence-based and bounded:

- `linux.procfs` currently means the expected process-stat path
  `/proc/self/stat` exists as a file. It does not claim every procfs feature is
  present or usable.
- `linux.cgroup-v2` currently means `/sys/fs/cgroup/cgroup.controllers` exists
  as a file. It does not claim particular controller availability, delegation,
  writability, container permissions, or systemd integration.

Linux OS identity alone does not imply every Linux facility.

Public helper functions expose the stable IDs so future ecosystem code can use
Fabric's open `HostFacilityId` model without copying string literals.

## Requirements

The crate provides reusable requirements:

- `linux_requirement()`
- `linux_procfs_requirement()`
- `linux_cgroup_v2_requirement()`

A future Adapter may use these to declare environment compatibility. A
requirement says what environment is compatible; it does not choose an Adapter
or encode realization policy, placement preference, or machine ranking.

## Boundaries

Docker is not a Host. Docker installation or availability is not detected here.
A future execution/runtime capability may use Docker as realization machinery,
but that is separate from Linux Host facts.

This crate deliberately does not detect or model Podman, systemd, GPUs, CPU
topology, RAM, disks, network interfaces, cloud metadata, hostname identity,
machine identity, remote hosts, schedulers, placement, Oracle concepts, or a
Host registry.

Future Linux facts should be added only when a consumer needs bounded
environment truth. Kernel version, cgroup controller detail, namespace state,
virtualization/KVM, container tooling, hardware inventory, and network inventory
are possible future facilities or separate concerns; none are implied by the
current descriptor.
