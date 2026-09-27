# Ed25519 Signing Example

This runnable example materializes an ephemeral Ed25519 signer, uses an
example-owned Component to sign application bytes, builds an example-owned
signed envelope, and verifies the public signature evidence.

Run from the repository root:

```sh
cargo run -p fabric-ecosystem-example-ed25519-signing
```

Expected output includes:

```text
signed payload: release payload
signature verified: true
```

Uses:

- `fabric-package-security-ed25519`
- `ephemeral_ed25519_signer("release")`

The signed envelope is application/example behavior. The example does not
export it as reusable ecosystem semantics and never prints private key material.

Source map:

- `src/main.rs`: scenario orchestration, verification, and output.
- `src/app.rs`: example-owned signing Component and signed-envelope shape.
