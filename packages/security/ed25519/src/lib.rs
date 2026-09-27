//! Ed25519 signing capability for Fabric.
//!
//! This package is deliberately Ed25519-specific. It owns public evidence
//! types, verification, the `Ed25519Signer` Resource, and an ephemeral signing
//! realization. Consumers own payload meaning, signed-envelope shape,
//! authorization, identity, and storage policy.

mod authoring;
mod ephemeral;
mod error;
mod public_key;
mod resource;
mod signature;
mod verification;

pub use authoring::ephemeral_ed25519_signer;
pub use ephemeral::EphemeralEd25519Signer;
pub use error::{
    Ed25519SigningError, Ed25519SigningErrorKind, Ed25519ValueError, Ed25519ValueErrorKind,
};
pub use public_key::Ed25519PublicKey;
pub use resource::Ed25519Signer;
pub use signature::Ed25519Signature;
pub use verification::verify_ed25519_signature;
