mod support;

use fabric::prelude::*;
use fabric_package_security_ed25519::{
    verify_ed25519_signature, Ed25519PublicKey, Ed25519Signature, Ed25519Signer,
    Ed25519SigningError,
};
use futures::executor::block_on;
use support::fabric::{activate, started_instance};

#[derive(Clone, Debug, PartialEq, Eq)]
struct SignedEnvelope {
    payload: Vec<u8>,
    public_key: Ed25519PublicKey,
    signature: Ed25519Signature,
}

fabric::component! {
    SigningConsumer {
        id: "onoal.test.third-party.security.signing-consumer";

        relations {
            requires {
                signer: Ed25519Signer;
            }
        }

        api {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<SignedEnvelope, Ed25519SigningError>;
        }

        runtime {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<SignedEnvelope, Ed25519SigningError> {
                let public_key = resolve_resource(self.relations().signer.public_key())?;
                let signature = resolve_resource(self.relations().signer.sign(payload.clone()))?;
                Ok(SignedEnvelope { payload, public_key, signature })
            }
        }
    }
}

fn signing_consumer(signer_name: &'static str) -> impl IntoFabricContribution {
    let signer = Ed25519Signer::select(signer_name).expect("signer selection");
    let component = SigningConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("signer").expect("role"),
            fabric::authoring::Requires::<Ed25519Signer>::provisional(),
        ),
        &signer,
    );
    FabricContribution::new().component(component)
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("third-party Ed25519 resource operation unexpectedly yielded")
        }
    }
}

#[test]
fn external_consumer_owns_signed_envelope_and_verifies_public_evidence() {
    let composition = Fabric::new("onoal.test.third-party.security")
        .expect("fabric")
        .with(fabric_package_security_ed25519::ephemeral_ed25519_signer(
            "release",
        ))
        .with(signing_consumer("release"))
        .build()
        .expect("composition");
    let instance = started_instance(&composition, "onoal.test.third-party.security.instance");
    let consumer = activate::<SigningConsumer>(&instance);

    let envelope = block_on(consumer.sign_payload(b"third-party-signed".to_vec()))
        .expect("exercise")
        .expect("signed envelope");

    assert!(verify_ed25519_signature(
        &envelope.public_key,
        &envelope.payload,
        &envelope.signature,
    ));
    assert!(!verify_ed25519_signature(
        &envelope.public_key,
        b"wrong-message",
        &envelope.signature,
    ));
}
