# Hosts

`hosts/` is the artifact family for reusable Host and environment truth.

Host artifacts are not capability Packages and are not Fabric semantic
participants:

```text
Host != Resource
Host != System
Host != Component
Host != Adapter
```

A future Host artifact may provide reusable authoring, detection, or
description of environment facts through Fabric's existing Host model. Examples
that may later be earned include Linux, macOS, Windows, and synthetic test
hosts.

The expected future shape is:

```text
hosts/<host>/
├── Cargo.toml
├── README.md
├── src/
│   └── lib.rs
└── tests/
```

Do not add a Host artifact until it has real source behavior to own. This
directory establishes the repository home and ownership law only.
