use ed25519_dalek::{VerifyingKey, PUBLIC_KEY_LENGTH};

use crate::Ed25519ValueError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519PublicKey {
    bytes: [u8; PUBLIC_KEY_LENGTH],
}

impl Ed25519PublicKey {
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_LENGTH]) -> Result<Self, Ed25519ValueError> {
        VerifyingKey::from_bytes(&bytes)
            .map_err(|error| Ed25519ValueError::invalid_public_key(error.to_string()))?;
        Ok(Self { bytes })
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, Ed25519ValueError> {
        let fixed: [u8; PUBLIC_KEY_LENGTH] = bytes.try_into().map_err(|_| {
            Ed25519ValueError::invalid_public_key(format!(
                "expected {PUBLIC_KEY_LENGTH} bytes, received {}",
                bytes.len()
            ))
        })?;
        Self::from_bytes(fixed)
    }

    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.bytes
    }

    pub(crate) fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey::from_bytes(&self.bytes)
            .expect("Ed25519PublicKey preserves verifying-key validity")
    }
}

impl From<VerifyingKey> for Ed25519PublicKey {
    fn from(value: VerifyingKey) -> Self {
        Self {
            bytes: value.to_bytes(),
        }
    }
}
