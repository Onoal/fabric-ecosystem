//! Ed25519 signing package for Fabric.
//!
//! This package models a concrete signing capability. The private key is live
//! adapter state for one materialized generation; it is not Composition truth,
//! identity, authority, certificate material, or durable key storage.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use ed25519_dalek::{
    Signature as DalekSignature, Signer, SigningKey, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH,
    SIGNATURE_LENGTH,
};
use fabric::prelude::*;
use rand_core::OsRng;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ed25519SigningErrorKind {
    NotStarted,
    Stopped,
    SigningFailed,
    InvalidPublicKey,
    InvalidSignature,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519SigningError {
    pub kind: Ed25519SigningErrorKind,
    pub detail: String,
}

impl Ed25519SigningError {
    fn not_started() -> Self {
        Self {
            kind: Ed25519SigningErrorKind::NotStarted,
            detail: "ed25519 signer has not started".to_owned(),
        }
    }

    fn stopped() -> Self {
        Self {
            kind: Ed25519SigningErrorKind::Stopped,
            detail: "ed25519 signer generation is stopped".to_owned(),
        }
    }

    fn invalid_public_key(detail: impl Into<String>) -> Self {
        Self {
            kind: Ed25519SigningErrorKind::InvalidPublicKey,
            detail: detail.into(),
        }
    }

    fn invalid_signature(detail: impl Into<String>) -> Self {
        Self {
            kind: Ed25519SigningErrorKind::InvalidSignature,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Ed25519SigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for Ed25519SigningError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519PublicKey {
    bytes: [u8; PUBLIC_KEY_LENGTH],
}

impl Ed25519PublicKey {
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_LENGTH]) -> Result<Self, Ed25519SigningError> {
        VerifyingKey::from_bytes(&bytes)
            .map_err(|error| Ed25519SigningError::invalid_public_key(error.to_string()))?;
        Ok(Self { bytes })
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, Ed25519SigningError> {
        let fixed: [u8; PUBLIC_KEY_LENGTH] = bytes.try_into().map_err(|_| {
            Ed25519SigningError::invalid_public_key(format!(
                "expected {PUBLIC_KEY_LENGTH} bytes, received {}",
                bytes.len()
            ))
        })?;
        Self::from_bytes(fixed)
    }

    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.bytes
    }

    fn verifying_key(&self) -> Result<VerifyingKey, Ed25519SigningError> {
        VerifyingKey::from_bytes(&self.bytes)
            .map_err(|error| Ed25519SigningError::invalid_public_key(error.to_string()))
    }
}

impl From<VerifyingKey> for Ed25519PublicKey {
    fn from(value: VerifyingKey) -> Self {
        Self {
            bytes: value.to_bytes(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519Signature {
    bytes: [u8; SIGNATURE_LENGTH],
}

impl Ed25519Signature {
    pub fn from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Self { bytes }
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, Ed25519SigningError> {
        let signature = DalekSignature::from_slice(bytes)
            .map_err(|error| Ed25519SigningError::invalid_signature(error.to_string()))?;
        Ok(Self {
            bytes: signature.to_bytes(),
        })
    }

    pub fn as_bytes(&self) -> &[u8; SIGNATURE_LENGTH] {
        &self.bytes
    }

    fn dalek_signature(&self) -> DalekSignature {
        DalekSignature::from_bytes(&self.bytes)
    }
}

pub fn verify_ed25519_signature(
    public_key: &Ed25519PublicKey,
    message: &[u8],
    signature: &Ed25519Signature,
) -> Result<bool, Ed25519SigningError> {
    let verifying_key = public_key.verifying_key()?;
    Ok(verifying_key
        .verify(message, &signature.dalek_signature())
        .is_ok())
}

pub struct EphemeralEd25519SignerState {
    signing_key: Mutex<Option<SigningKey>>,
    live: AtomicBool,
    stopped: AtomicBool,
}

impl Default for EphemeralEd25519SignerState {
    fn default() -> Self {
        Self {
            signing_key: Mutex::new(None),
            live: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        }
    }
}

impl EphemeralEd25519SignerState {
    fn current_key(&self) -> Result<SigningKey, Ed25519SigningError> {
        if !self.live.load(Ordering::SeqCst) {
            return if self.stopped.load(Ordering::SeqCst) {
                Err(Ed25519SigningError::stopped())
            } else {
                Err(Ed25519SigningError::not_started())
            };
        }
        self.signing_key
            .lock()
            .expect("ed25519 signing key")
            .clone()
            .ok_or_else(Ed25519SigningError::not_started)
    }
}

fabric::resource! {
    pub Ed25519Signer {
        id: "onoal.package.security.ed25519.signer";

        api {
            fn public_key(&self) -> Result<Ed25519PublicKey, Ed25519SigningError>;
            fn sign(&self, message: Vec<u8>) -> Result<Ed25519Signature, Ed25519SigningError>;
        }
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
                Ok(Ed25519PublicKey::from(
                    self.state.get().current_key()?.verifying_key(),
                ))
            }

            fn sign(&self, message: Vec<u8>) -> Result<Ed25519Signature, Ed25519SigningError> {
                let signing_key = self.state.get().current_key()?;
                let signature: DalekSignature = signing_key.sign(&message);
                Ok(Ed25519Signature::from_bytes(signature.to_bytes()))
            }
        }

        lifecycle {
            start {
                let mut rng = OsRng;
                let signing_key = SigningKey::generate(&mut rng);
                *self
                    .state
                    .get()
                    .signing_key
                    .lock()
                    .expect("ed25519 signing key") = Some(signing_key);
                self.state.get().stopped.store(false, Ordering::SeqCst);
                self.state.get().live.store(true, Ordering::SeqCst);
                Ok(())
            }

            stop {
                self.state.get().live.store(false, Ordering::SeqCst);
                *self
                    .state
                    .get()
                    .signing_key
                    .lock()
                    .expect("ed25519 signing key") = None;
                self.state.get().stopped.store(true, Ordering::SeqCst);
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedPayload {
    pub payload: Vec<u8>,
    pub public_key: Ed25519PublicKey,
    pub signature: Ed25519Signature,
}

fabric::component! {
    pub SignedPayloadProducer {
        id: "onoal.package.security.ed25519.signed-payload-producer";

        relations {
            requires {
                signer: Ed25519Signer;
            }
        }

        api {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<SignedPayload, Ed25519SigningError>;
        }

        runtime {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<SignedPayload, Ed25519SigningError> {
                let public_key = self.relations().signer.public_key()?;
                let signature = self.relations().signer.sign(payload.clone())?;
                Ok(SignedPayload {
                    payload,
                    public_key,
                    signature,
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualSignedPayloads {
    pub release: SignedPayload,
    pub audit: SignedPayload,
}

fabric::component! {
    pub DualSignedPayloadProducer {
        id: "onoal.package.security.ed25519.dual-signed-payload-producer";

        relations {
            requires {
                release: Ed25519Signer;
                audit: Ed25519Signer;
            }
        }

        api {
            fn sign_with_both(&self, payload: Vec<u8>) -> Result<DualSignedPayloads, Ed25519SigningError>;
        }

        runtime {
            fn sign_with_both(&self, payload: Vec<u8>) -> Result<DualSignedPayloads, Ed25519SigningError> {
                let release_public_key = self.relations().release.public_key()?;
                let release_signature = self.relations().release.sign(payload.clone())?;
                let audit_public_key = self.relations().audit.public_key()?;
                let audit_signature = self.relations().audit.sign(payload.clone())?;
                Ok(DualSignedPayloads {
                    release: SignedPayload {
                        payload: payload.clone(),
                        public_key: release_public_key,
                        signature: release_signature,
                    },
                    audit: SignedPayload {
                        payload,
                        public_key: audit_public_key,
                        signature: audit_signature,
                    },
                })
            }
        }
    }
}

pub fn ephemeral_ed25519_signer(name: &'static str) -> impl IntoFabricContribution {
    let selected = Ed25519Signer::select(name).expect("valid Ed25519 signer resource name");
    FabricContribution::new().resource(
        selected
            .using(EphemeralEd25519Signer::new())
            .expect("EphemeralEd25519Signer supports Ed25519Signer"),
    )
}

pub fn signed_payload_producer(signer_name: &'static str) -> impl IntoFabricContribution {
    let signer = Ed25519Signer::select(signer_name).expect("valid Ed25519 signer resource name");
    let component = SignedPayloadProducer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("signer").expect("role"),
            fabric::authoring::Requires::<Ed25519Signer>::provisional(),
        ),
        &signer,
    );

    FabricContribution::new().component(component)
}

pub fn dual_signed_payload_producer(
    release_signer_name: &'static str,
    audit_signer_name: &'static str,
) -> impl IntoFabricContribution {
    let release =
        Ed25519Signer::select(release_signer_name).expect("valid release signer resource name");
    let audit = Ed25519Signer::select(audit_signer_name).expect("valid audit signer resource name");
    let component = DualSignedPayloadProducer::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("release").expect("role"),
                fabric::authoring::Requires::<Ed25519Signer>::provisional(),
            ),
            &release,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("audit").expect("role"),
                fabric::authoring::Requires::<Ed25519Signer>::provisional(),
            ),
            &audit,
        );

    FabricContribution::new().component(component)
}

pub fn ed25519_signing_capability(signer_name: &'static str) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(ephemeral_ed25519_signer(signer_name))
        .with(signed_payload_producer(signer_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    fn composition(id: &str) -> Composition {
        Fabric::new(id)
            .expect("fabric")
            .with(ed25519_signing_capability("release"))
            .build()
            .expect("composition")
    }

    fn started_instance(composition: &Composition, id: &str) -> Instance {
        let mut instance = composition
            .materialize_on(id, &HostDescriptor::native())
            .expect("instance");
        instance.start().expect("start");
        instance
    }

    fn activate<C: fabric::authoring::ComponentDefinition>(
        instance: &Instance,
    ) -> BoundComponent<'_, C> {
        let component = instance.component::<C>().expect("component");
        component.reconcile().expect("reconcile");
        component
    }

    #[test]
    fn composition_declares_signer_and_ephemeral_realization_without_secret_truth() {
        let helper = Fabric::new("onoal.package.test.security.inspect.helper")
            .expect("fabric")
            .with(ephemeral_ed25519_signer("release"))
            .build()
            .expect("composition");
        let explicit = Fabric::new("onoal.package.test.security.inspect.explicit")
            .expect("fabric")
            .resource(
                Ed25519Signer::select("release")
                    .expect("signer")
                    .using(EphemeralEd25519Signer::new())
                    .expect("adapter"),
            )
            .build()
            .expect("composition");

        let helper_signer = helper.resources().next().expect("helper signer");
        let explicit_signer = explicit.resources().next().expect("explicit signer");
        assert_eq!(helper_signer.resource_id(), explicit_signer.resource_id());
        assert_eq!(helper_signer.name(), explicit_signer.name());
        assert_eq!(
            helper_signer
                .realization()
                .adapter_definition_id()
                .expect("helper adapter"),
            explicit_signer
                .realization()
                .adapter_definition_id()
                .expect("explicit adapter")
        );
        assert_eq!(helper.components().count(), 0);
        assert_eq!(helper.resources().count(), 1);
    }

    #[test]
    fn component_signs_arbitrary_bytes_and_pure_verification_checks_evidence() {
        let composition = composition("onoal.package.test.security.sign");
        let instance = started_instance(&composition, "onoal.package.test.security.sign.instance");
        let producer = activate::<SignedPayloadProducer>(&instance);
        let signed = block_on(producer.sign_payload(b"arbitrary bytes".to_vec()))
            .expect("invoke")
            .expect("signing");

        assert_eq!(signed.payload, b"arbitrary bytes".to_vec());
        assert!(
            verify_ed25519_signature(&signed.public_key, &signed.payload, &signed.signature,)
                .expect("verify")
        );
        assert!(
            !verify_ed25519_signature(&signed.public_key, b"wrong message", &signed.signature,)
                .expect("wrong message")
        );
        let same_signer = block_on(producer.sign_payload(b"other".to_vec()))
            .expect("second invoke")
            .expect("second signing")
            .public_key;
        assert!(
            verify_ed25519_signature(&same_signer, &signed.payload, &signed.signature,)
                .expect("same occurrence key verifies")
        );
    }

    #[test]
    fn multiple_occurrences_have_independent_live_keys_and_roles() {
        let composition = Fabric::new("onoal.package.test.security.occurrences")
            .expect("fabric")
            .with(ephemeral_ed25519_signer("release"))
            .with(ephemeral_ed25519_signer("audit"))
            .with(dual_signed_payload_producer("release", "audit"))
            .build()
            .expect("composition");
        assert_eq!(composition.resources().count(), 2);
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "release"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "audit"));

        let instance = started_instance(
            &composition,
            "onoal.package.test.security.occurrences.instance",
        );
        let producer = activate::<DualSignedPayloadProducer>(&instance);
        let signed = block_on(producer.sign_with_both(b"release artifact".to_vec()))
            .expect("invoke")
            .expect("sign both");

        assert_ne!(signed.release.public_key, signed.audit.public_key);
        assert!(verify_ed25519_signature(
            &signed.release.public_key,
            &signed.release.payload,
            &signed.release.signature,
        )
        .expect("release verifies"));
        assert!(verify_ed25519_signature(
            &signed.audit.public_key,
            &signed.audit.payload,
            &signed.audit.signature,
        )
        .expect("audit verifies"));
        assert!(!verify_ed25519_signature(
            &signed.audit.public_key,
            &signed.release.payload,
            &signed.release.signature,
        )
        .expect("wrong key"));
    }

    #[test]
    fn multi_instance_and_fresh_generation_receive_fresh_live_key_state() {
        let composition = composition("onoal.package.test.security.instances");
        let mut first =
            started_instance(&composition, "onoal.package.test.security.instances.same");
        let second = started_instance(&composition, "onoal.package.test.security.instances.other");

        let first_signed = {
            let first_producer = activate::<SignedPayloadProducer>(&first);
            block_on(first_producer.sign_payload(b"first".to_vec()))
                .expect("first invoke")
                .expect("first sign")
        };
        let second_producer = activate::<SignedPayloadProducer>(&second);
        let second_signed = block_on(second_producer.sign_payload(b"second".to_vec()))
            .expect("second invoke")
            .expect("second sign");
        assert_ne!(first_signed.public_key, second_signed.public_key);

        first.stop().expect("stop first");
        let stopped_producer = first
            .component::<SignedPayloadProducer>()
            .expect("stopped producer handle");
        assert!(block_on(stopped_producer.sign_payload(b"after-stop".to_vec())).is_err());

        let fresh = started_instance(&composition, "onoal.package.test.security.instances.same");
        let fresh_producer = activate::<SignedPayloadProducer>(&fresh);
        let fresh_signed = block_on(fresh_producer.sign_payload(b"fresh".to_vec()))
            .expect("fresh invoke")
            .expect("fresh sign");
        assert_ne!(first_signed.public_key, fresh_signed.public_key);
        assert!(verify_ed25519_signature(
            &fresh_signed.public_key,
            &fresh_signed.payload,
            &fresh_signed.signature,
        )
        .expect("fresh verifies"));
    }

    #[test]
    fn invalid_public_key_and_signature_inputs_are_bounded_errors() {
        assert!(matches!(
            Ed25519PublicKey::from_slice(&[0; 12]),
            Err(Ed25519SigningError {
                kind: Ed25519SigningErrorKind::InvalidPublicKey,
                ..
            })
        ));
        assert!(matches!(
            Ed25519Signature::from_slice(&[0; 12]),
            Err(Ed25519SigningError {
                kind: Ed25519SigningErrorKind::InvalidSignature,
                ..
            })
        ));
    }
}
