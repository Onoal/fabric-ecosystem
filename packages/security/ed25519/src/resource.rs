use crate::{Ed25519PublicKey, Ed25519Signature, Ed25519SigningError};

fabric::resource! {
    pub Ed25519Signer {
        id: "onoal.package.security.ed25519.signer";

        api {
            async fn public_key(&self) -> Result<Ed25519PublicKey, Ed25519SigningError>;
            async fn sign(&self, message: Vec<u8>) -> Result<Ed25519Signature, Ed25519SigningError>;
        }
    }
}
