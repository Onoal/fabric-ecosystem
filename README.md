# Fabric Packages

`fabric-packages` contains official maintained candidate/reference packages built with
Fabric. These crates are ordinary Fabric ecosystem packages maintained by Onoal;
they do not define Fabric Core and they have no privileged semantic status over
third-party packages built on the same kernel.

The repository currently carries four package and integration groups:

- concrete Resources
- concrete Adapters
- concrete Components
- concrete integration witness suites

Generic Fabric kernel crates such as `fabric-core`, `fabric-resource`,
`fabric-component`, `fabric-host`, and related horizontal APIs are consumed from
[`Onoal/fabric`](https://github.com/Onoal/fabric) at a pinned Git revision.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the repository boundary and
[PROVENANCE.md](PROVENANCE.md) for the RB0 copy record.
