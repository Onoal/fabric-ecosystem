use fabric::prelude::*;
use fabric_package_security_ed25519::{
    Ed25519PublicKey, Ed25519Signature, Ed25519Signer, Ed25519SigningError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SignedEnvelope {
    pub(crate) payload: Vec<u8>,
    pub(crate) public_key: Ed25519PublicKey,
    pub(crate) signature: Ed25519Signature,
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
            fn sign_payload(&self, payload: Vec<u8>) -> Result<SignedEnvelope, Ed25519SigningError>;
        }

        runtime {
            fn sign_payload(&self, payload: Vec<u8>) -> Result<SignedEnvelope, Ed25519SigningError> {
                let public_key = self.relations().signer.public_key()?;
                let signature = self.relations().signer.sign(payload.clone())?;
                Ok(SignedEnvelope {
                    payload,
                    public_key,
                    signature,
                })
            }
        }
    }
}

pub(crate) fn signing_app(signer_name: &'static str) -> impl IntoFabricContribution {
    let signer = Ed25519Signer::select(signer_name).expect("valid Ed25519 signer resource name");
    let component = ExampleSigningApp::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("signer").expect("role"),
            fabric::authoring::Requires::<Ed25519Signer>::provisional(),
        ),
        &signer,
    );
    FabricContribution::new().component(component)
}
