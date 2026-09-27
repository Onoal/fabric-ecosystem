use ed25519_dalek::Verifier;

use crate::{Ed25519PublicKey, Ed25519Signature};

pub fn verify_ed25519_signature(
    public_key: &Ed25519PublicKey,
    message: &[u8],
    signature: &Ed25519Signature,
) -> bool {
    public_key
        .verifying_key()
        .verify(message, &signature.dalek_signature())
        .is_ok()
}
