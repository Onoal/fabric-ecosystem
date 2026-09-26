# Examples

Examples are teaching and demonstration artifacts.

They may be explicit, redundant, or narrowly scoped when that makes Fabric and
ecosystem package usage easier to understand. They are not reusable packages and
not verification fixtures.

Examples may use Packages or reusable Compositions, but they do not own
reusable ecosystem semantics merely because they are runnable.

- `key-value-quickstart`: a compact storage package quickstart.
- `ed25519-signing`: a compact security package example that materializes an
  ephemeral signer, signs bytes through a Component, verifies the public
  evidence, and stops the Instance.
- `http-server`: a compact Composition example that consumes the reusable HTTP
  server assembly, materializes it, discovers the bound TCP address, and serves
  one real loopback HTTP request.
- `local-backend`: a compact nested-Composition example that consumes the local
  backend foundation, adds application-owned database/log behavior, and serves
  one HTTP response from local data.
- `observed-queue`: a compact observability example showing application-owned
  instrumentation over Queue behavior with success/failure counters.
