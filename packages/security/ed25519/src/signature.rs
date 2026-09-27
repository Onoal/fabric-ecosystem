use ed25519_dalek::{Signature as DalekSignature, SIGNATURE_LENGTH};

use crate::Ed25519ValueError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519Signature {
    bytes: [u8; SIGNATURE_LENGTH],
}

impl Ed25519Signature {
    pub fn from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Self { bytes }
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, Ed25519ValueError> {
        let signature = DalekSignature::from_slice(bytes)
            .map_err(|error| Ed25519ValueError::invalid_signature(error.to_string()))?;
        Ok(Self {
            bytes: signature.to_bytes(),
        })
    }

    pub fn as_bytes(&self) -> &[u8; SIGNATURE_LENGTH] {
        &self.bytes
    }

    pub(crate) fn dalek_signature(&self) -> DalekSignature {
        DalekSignature::from_bytes(&self.bytes)
    }
}
