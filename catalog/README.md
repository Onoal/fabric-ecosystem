# Catalog

`catalog/` is reserved for discovery, index, and navigation artifacts.

The Catalog is not a Fabric semantic model and not a package manifest schema.
It must not become the source of truth for:

- Resource definitions;
- System definitions;
- Component declarations;
- Adapter compatibility;
- Config;
- Relations;
- Composition graphs;
- runtime state.

Fabric and Rust source remain authoritative. Future catalog views may be
generated or curated to help humans, CLIs, websites, documentation, or
development agents discover ecosystem artifacts.

No catalog generator or metadata schema is established in M1. Add one only
after enough real package, Host, Composition, example, and test pressure
exists.
