# Examples

Examples are runnable teaching applications.

A top-level Fabric Ecosystem example is a runnable application first. Its tests
prove that the executable works; the example does not exist primarily to satisfy
tests. Run examples with `cargo run -p <example-package>`.

Examples may be explicit, redundant, or narrowly scoped when that makes Fabric
and ecosystem package usage easier to understand. They are not reusable
libraries, package APIs, or verification fixtures.

Examples may use Packages or reusable Compositions, but they do not own
reusable ecosystem semantics merely because they are runnable.
Example-owned behavior is application behavior, not reusable ecosystem
semantics.

- [`key-value-quickstart`](key-value-quickstart/README.md): a compact storage
  package quickstart showing
  application-owned behavior requiring the `KeyValue` Resource directly.
- [`ed25519-signing`](ed25519-signing/README.md): a compact security package
  example that materializes an ephemeral signer, signs bytes through a
  Component, verifies the public evidence, and stops the Instance.
- [`http-server`](http-server/README.md): a compact Composition example that
  consumes the reusable HTTP server assembly, materializes it, discovers the
  bound TCP address, and serves one real loopback HTTP request.
- [`local-backend`](local-backend/README.md): a compact nested-Composition
  example that consumes the local backend foundation, adds application-owned
  database/log behavior, and serves one HTTP response from local data.
- [`observed-queue`](observed-queue/README.md): a compact observability example
  showing application-owned instrumentation over Queue behavior with
  success/failure counters.
