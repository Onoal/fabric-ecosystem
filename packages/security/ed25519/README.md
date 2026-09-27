# Ed25519

This package owns a concrete Ed25519 signing capability for Fabric.

It is intentionally algorithm-specific. It is not a generic cryptography
provider, generic signer abstraction, key store, PKI model, identity system, or
authority system.

## Semantic Boundary

`Ed25519Signer` is a named Resource that can:

- return the public verifying key for the live signer occurrence;
- sign opaque message bytes.

Messages are `Vec<u8>`. The package does not interpret JSON, documents,
claims, hashes, manifests, identities, certificates, or application payloads.

Verification is a pure function:

```rust
verify_ed25519_signature(public_key, message, signature) -> bool
```

`true` means the signature verifies for that exact public key and message.
`false` means it does not. Verification does not mean the message is true, the
key is trusted, the signer is authorized, identity is known, freshness is
guaranteed, or replay has been prevented.

## Evidence Values

`Ed25519PublicKey` preserves a validated fixed-width Ed25519 public-key value.
It does not expose `ed25519_dalek::VerifyingKey`.

`Ed25519Signature` preserves fixed-width signature bytes. Correct length and
representation are not the same as validity for a particular message/public key;
verification determines that relation.

Value construction failures use `Ed25519ValueError`.

Signing-operation failures use `Ed25519SigningError`. The current ephemeral
software signer is expected to succeed while live, but future realizations such
as HSMs, secure enclaves, device signers, or remote signers can fail a signing
operation honestly.

## Ephemeral Realization

`EphemeralEd25519Signer` generates a fresh Ed25519 signing key when a Fabric
generation starts.

Lifecycle:

```text
start generation -> generate fresh signing key
live generation  -> same occurrence uses the same key
stop generation  -> remove package-owned live key state
fresh generation -> generate fresh key state
```

The private key is realization runtime state. It is not Composition truth,
MaterializationProfile truth, public Resource state, identity, authority,
certificate material, or durable storage.

Normal `public_key()` and `sign()` operations do not clone the package-owned
`SigningKey` out of state. They operate on the key while it is borrowed under
the state lock.

The dependency configuration enables `ed25519-dalek`'s `zeroize` feature; in
the resolved `ed25519-dalek 2.2` source, `SigningKey` implements zeroization on
drop. This package relies on that dependency behavior when the live key is
removed from state, but it does not claim broader process-memory erasure
guarantees.

Use `ephemeral_ed25519_signer("release")` to contribute one named signer
occurrence. Multiple named occurrences have independent live key state.

## Consumer-Owned Envelopes

Consumers own what a payload means and how public key, signature, payload,
metadata, provenance, and storage are packaged. This package does not define a
generic signed-envelope schema.

## Extension Path

Future pressure may add imported or persistent key realizations, HSM/secure
enclave/remote signer realizations, key identifiers, rotation policy in a
higher key-management capability, or a generic signing abstraction after more
algorithms are earned.

## Non-Goals

- generic cryptography provider
- generic `Signer` abstraction
- certificate or PKI model
- identity, authority, or trust decision
- key persistence, export, recovery, escrow, or secret manager
- HSM, secure enclave, or remote signer implementation
- key rotation policy
- threshold or multi-signature policy
- payload serialization or hashing policy
- signed-envelope schema
- audit evidence
