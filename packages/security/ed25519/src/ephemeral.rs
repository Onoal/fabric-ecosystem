use std::sync::Mutex;

use ed25519_dalek::{Signature as DalekSignature, Signer, SigningKey};
use rand_core::OsRng;

use crate::{Ed25519PublicKey, Ed25519Signature, Ed25519Signer, Ed25519SigningError};

#[derive(Default)]
struct EphemeralEd25519SignerState {
    signing_key: Mutex<Option<SigningKey>>,
}

impl EphemeralEd25519SignerState {
    fn with_signing_key<T>(
        &self,
        operation: impl FnOnce(&SigningKey) -> T,
    ) -> Result<T, Ed25519SigningError> {
        let guard = self.signing_key.lock().map_err(|_| {
            Ed25519SigningError::signing_failed("ed25519 signing key lock poisoned")
        })?;
        let signing_key = guard.as_ref().ok_or_else(|| {
            Ed25519SigningError::signing_failed("ed25519 signing key unavailable")
        })?;
        Ok(operation(signing_key))
    }

    fn replace_key(&self, signing_key: SigningKey) -> Result<(), fabric::core::ModuleError> {
        *self.signing_key.lock().map_err(|_| {
            fabric::core::ModuleError::new("ed25519 signing key lock poisoned during start")
        })? = Some(signing_key);
        Ok(())
    }

    fn discard_key(&self) -> Result<(), fabric::core::ModuleError> {
        *self.signing_key.lock().map_err(|_| {
            fabric::core::ModuleError::new("ed25519 signing key lock poisoned during stop")
        })? = None;
        Ok(())
    }
}

fabric::adapter! {
    pub EphemeralEd25519Signer for Ed25519Signer {
        id: "onoal.package.security.ed25519.signer.ephemeral";

        state {
            EphemeralEd25519SignerState = EphemeralEd25519SignerState::default();
        }

        runtime {
            fn public_key(&self) -> Result<Ed25519PublicKey, Ed25519SigningError> {
                self.state
                    .get()
                    .with_signing_key(|signing_key| Ed25519PublicKey::from(signing_key.verifying_key()))
            }

            fn sign(&self, message: Vec<u8>) -> Result<Ed25519Signature, Ed25519SigningError> {
                self.state.get().with_signing_key(|signing_key| {
                    let signature: DalekSignature = signing_key.sign(&message);
                    Ed25519Signature::from_bytes(signature.to_bytes())
                })
            }
        }

        lifecycle {
            start {
                let mut rng = OsRng;
                self.state
                    .get()
                    .replace_key(SigningKey::generate(&mut rng))
            }

            stop {
                self.state.get().discard_key()
            }
        }
    }
}
