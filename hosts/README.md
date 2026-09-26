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

Host artifacts may provide reusable authoring, detection, or description of
environment facts through Fabric's existing Host model. `hosts/linux` is the
first real artifact and owns Linux detection, Linux facility identifiers, and
Linux Host requirements.

The expected future shape is:

```text
hosts/<host>/
├── Cargo.toml
├── README.md
├── src/
│   └── lib.rs
└── tests/
```

Do not add a Host artifact until it has real source behavior to own.
