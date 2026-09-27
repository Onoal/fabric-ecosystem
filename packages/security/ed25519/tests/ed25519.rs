use fabric::prelude::*;
use fabric_package_security_ed25519::{
    ephemeral_ed25519_signer, verify_ed25519_signature, Ed25519PublicKey, Ed25519Signature,
    Ed25519Signer, Ed25519SigningError, Ed25519ValueError, Ed25519ValueErrorKind,
    EphemeralEd25519Signer,
};
use futures::executor::block_on;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestSignedPayload {
    payload: Vec<u8>,
    public_key: Ed25519PublicKey,
    signature: Ed25519Signature,
}

fabric::component! {
    TestSigningApp {
        id: "onoal.package.security.ed25519.test-signing-app";

        relations {
            requires {
                signer: Ed25519Signer;
            }
        }

        api {
            fn public_key(&self) -> Result<Ed25519PublicKey, Ed25519SigningError>;
            fn sign_payload(&self, payload: Vec<u8>) -> Result<TestSignedPayload, Ed25519SigningError>;
        }

        runtime {
            fn public_key(&self) -> Result<Ed25519PublicKey, Ed25519SigningError> {
                self.relations().signer.public_key()
            }

            fn sign_payload(&self, payload: Vec<u8>) -> Result<TestSignedPayload, Ed25519SigningError> {
                let public_key = self.relations().signer.public_key()?;
                let signature = self.relations().signer.sign(payload.clone())?;
                Ok(TestSignedPayload {
                    payload,
                    public_key,
                    signature,
                })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestDualSignedPayloads {
    release: TestSignedPayload,
    audit: TestSignedPayload,
}

fabric::component! {
    TestDualSigningApp {
        id: "onoal.package.security.ed25519.test-dual-signing-app";

        relations {
            requires {
                release: Ed25519Signer;
                audit: Ed25519Signer;
            }
        }

        api {
            fn sign_with_both(&self, payload: Vec<u8>) -> Result<TestDualSignedPayloads, Ed25519SigningError>;
        }

        runtime {
            fn sign_with_both(&self, payload: Vec<u8>) -> Result<TestDualSignedPayloads, Ed25519SigningError> {
                let release_public_key = self.relations().release.public_key()?;
                let release_signature = self.relations().release.sign(payload.clone())?;
                let audit_public_key = self.relations().audit.public_key()?;
                let audit_signature = self.relations().audit.sign(payload.clone())?;
                Ok(TestDualSignedPayloads {
                    release: TestSignedPayload {
                        payload: payload.clone(),
                        public_key: release_public_key,
                        signature: release_signature,
                    },
                    audit: TestSignedPayload {
                        payload,
                        public_key: audit_public_key,
                        signature: audit_signature,
                    },
                })
            }
        }
    }
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

fn signing_app(signer_name: &'static str) -> impl IntoFabricContribution {
    let signer = Ed25519Signer::select(signer_name).expect("signer selection");
    let component = TestSigningApp::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("signer").expect("role"),
            fabric::authoring::Requires::<Ed25519Signer>::provisional(),
        ),
        &signer,
    );

    FabricContribution::new().component(component)
}

fn dual_signing_app(
    release_signer_name: &'static str,
    audit_signer_name: &'static str,
) -> impl IntoFabricContribution {
    let release = Ed25519Signer::select(release_signer_name).expect("release signer selection");
    let audit = Ed25519Signer::select(audit_signer_name).expect("audit signer selection");
    let component = TestDualSigningApp::define()
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

fn single_signer_composition(id: &str) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(ephemeral_ed25519_signer("release"))
        .with(signing_app("release"))
        .build()
        .expect("composition")
}

#[test]
fn composition_declares_signer_and_ephemeral_realization_without_secret_truth() {
    let helper = Fabric::new("onoal.package.test.security.ed25519.inspect.helper")
        .expect("fabric")
        .with(ephemeral_ed25519_signer("release"))
        .build()
        .expect("composition");
    let explicit = Fabric::new("onoal.package.test.security.ed25519.inspect.explicit")
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
fn public_key_and_signature_value_construction_are_bounded() {
    let composition = single_signer_composition("onoal.package.test.security.ed25519.values");
    let instance = started_instance(
        &composition,
        "onoal.package.test.security.ed25519.values.instance",
    );
    let app = activate::<TestSigningApp>(&instance);
    let signed = block_on(app.sign_payload(b"value checks".to_vec()))
        .expect("invoke")
        .expect("signing");

    assert!(Ed25519PublicKey::from_slice(signed.public_key.as_bytes()).is_ok());
    assert!(Ed25519Signature::from_slice(signed.signature.as_bytes()).is_ok());
    assert!(matches!(
        Ed25519PublicKey::from_slice(&[0; 12]),
        Err(Ed25519ValueError {
            kind: Ed25519ValueErrorKind::InvalidPublicKey,
            ..
        })
    ));
    assert!(matches!(
        Ed25519Signature::from_slice(&[0; 12]),
        Err(Ed25519ValueError {
            kind: Ed25519ValueErrorKind::InvalidSignature,
            ..
        })
    ));
}

#[test]
fn consumer_owned_component_signs_bytes_and_verification_checks_evidence() {
    let composition = single_signer_composition("onoal.package.test.security.ed25519.sign");
    let instance = started_instance(
        &composition,
        "onoal.package.test.security.ed25519.sign.instance",
    );
    let app = activate::<TestSigningApp>(&instance);
    let signed = block_on(app.sign_payload(b"arbitrary bytes".to_vec()))
        .expect("invoke")
        .expect("signing");

    assert_eq!(signed.payload, b"arbitrary bytes".to_vec());
    assert!(verify_ed25519_signature(
        &signed.public_key,
        &signed.payload,
        &signed.signature,
    ));
    assert!(!verify_ed25519_signature(
        &signed.public_key,
        b"wrong message",
        &signed.signature,
    ));

    let same_generation_key = block_on(app.public_key())
        .expect("public key invoke")
        .expect("public key");
    assert_eq!(signed.public_key, same_generation_key);
}

#[test]
fn multiple_occurrences_have_independent_live_keys_and_wrong_signer_fails() {
    let composition = Fabric::new("onoal.package.test.security.ed25519.occurrences")
        .expect("fabric")
        .with(ephemeral_ed25519_signer("release"))
        .with(ephemeral_ed25519_signer("audit"))
        .with(dual_signing_app("release", "audit"))
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
        "onoal.package.test.security.ed25519.occurrences.instance",
    );
    let app = activate::<TestDualSigningApp>(&instance);
    let signed = block_on(app.sign_with_both(b"release artifact".to_vec()))
        .expect("invoke")
        .expect("sign both");

    assert_ne!(signed.release.public_key, signed.audit.public_key);
    assert!(verify_ed25519_signature(
        &signed.release.public_key,
        &signed.release.payload,
        &signed.release.signature,
    ));
    assert!(verify_ed25519_signature(
        &signed.audit.public_key,
        &signed.audit.payload,
        &signed.audit.signature,
    ));
    assert!(!verify_ed25519_signature(
        &signed.audit.public_key,
        &signed.release.payload,
        &signed.release.signature,
    ));
}

#[test]
fn multi_instance_and_fresh_generation_receive_fresh_ephemeral_key_state() {
    let composition = single_signer_composition("onoal.package.test.security.ed25519.instances");
    let mut first = started_instance(
        &composition,
        "onoal.package.test.security.ed25519.instances.same",
    );
    let second = started_instance(
        &composition,
        "onoal.package.test.security.ed25519.instances.other",
    );

    let first_signed = {
        let first_app = activate::<TestSigningApp>(&first);
        block_on(first_app.sign_payload(b"first".to_vec()))
            .expect("first invoke")
            .expect("first sign")
    };
    let second_app = activate::<TestSigningApp>(&second);
    let second_signed = block_on(second_app.sign_payload(b"second".to_vec()))
        .expect("second invoke")
        .expect("second sign");
    assert_ne!(first_signed.public_key, second_signed.public_key);

    first.stop().expect("stop first");
    let stopped_app = first
        .component::<TestSigningApp>()
        .expect("stopped app handle");
    assert!(block_on(stopped_app.sign_payload(b"after-stop".to_vec())).is_err());

    let fresh = started_instance(
        &composition,
        "onoal.package.test.security.ed25519.instances.same",
    );
    let fresh_app = activate::<TestSigningApp>(&fresh);
    let fresh_signed = block_on(fresh_app.sign_payload(b"fresh".to_vec()))
        .expect("fresh invoke")
        .expect("fresh sign");
    assert_ne!(first_signed.public_key, fresh_signed.public_key);
    assert!(verify_ed25519_signature(
        &fresh_signed.public_key,
        &fresh_signed.payload,
        &fresh_signed.signature,
    ));
}
