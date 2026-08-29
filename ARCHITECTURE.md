# Architecture

`fabric-packages` is the maintained repository for first-party Fabric ecosystem
packages that are implemented using the Fabric kernel.

The repository boundary is:

- Fabric kernel lives in `Onoal/fabric`
- concrete candidate/reference packages live here
- third-party packages are architecturally equivalent to these first-party
  packages

This repository intentionally preserves the existing package semantics:

- `Worker`, `Server`, and `Process` remain distinct Resources
- `Authority` remains separate from the Cedar decision engine
- `Resource` remains separate from `Adapter`
- `Service` remains separate from `Server`
- `Binding` remains separate from `Projection`
- Host compatibility remains Adapter-owned

What is not here:

- no copied `core/`, `host/`, or `sdk/`
- no copied generic Resource machinery such as `binding`, `projection`,
  `resource`, or `registry`
- no copied generic Component kernel crate
- no STEL product packages

Concrete integration witnesses stay in this repository because they exercise the
copied Resources, Adapters, and Components as ecosystem packages rather than as
Fabric kernel code.
