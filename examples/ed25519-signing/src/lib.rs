//! Ed25519 signing example for Fabric Ecosystem.

use fabric::prelude::*;
use fabric_package_security_ed25519::{
    ephemeral_ed25519_signer, verify_ed25519_signature, Ed25519PublicKey, Ed25519Signature,
    Ed25519Signer, Ed25519SigningError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExampleSignedPayload {
    pub payload: Vec<u8>,
    pub public_key: Ed25519PublicKey,
    pub signature: Ed25519Signature,
}

fabric::component! {
    pub ExampleSigningApp {
        id: "fabric.ecosystem.example.ed25519-signing.app";

        relations {
            requires {
                signer: Ed25519Signer;
            }
        }

        api {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<ExampleSignedPayload, Ed25519SigningError>;
        }

        runtime {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<ExampleSignedPayload, Ed25519SigningError> {
                let public_key = self.relations().signer.public_key()?;
                let signature = self.relations().signer.sign(payload.clone())?;
                Ok(ExampleSignedPayload {
                    payload,
                    public_key,
                    signature,
                })
            }
        }
    }
}

pub fn signing_app(signer_name: &'static str) -> impl IntoFabricContribution {
    let signer = Ed25519Signer::select(signer_name).expect("signer selection");
    let component = ExampleSigningApp::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("signer").expect("role"),
            fabric::authoring::Requires::<Ed25519Signer>::provisional(),
        ),
        &signer,
    );
    FabricContribution::new().component(component)
}

pub fn run() -> Result<ExampleSignedPayload, Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.ed25519-signing")?
        .with(ephemeral_ed25519_signer("release"))
        .with(signing_app("release"))
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.ed25519-signing.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let app = instance.component::<ExampleSigningApp>()?;
    app.reconcile()?;
    let signed = futures::executor::block_on(app.sign_payload(b"release payload".to_vec()))??;
    assert!(verify_ed25519_signature(
        &signed.public_key,
        &signed.payload,
        &signed.signature,
    ));

    instance.stop()?;
    Ok(signed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_builds_materializes_signs_verifies_and_stops() {
        let signed = run().expect("ed25519 signing example");
        assert_eq!(signed.payload, b"release payload".to_vec());
    }
}
