//! Ed25519 signing example for Fabric Ecosystem.

use fabric::prelude::*;
use fabric_package_security_ed25519::{
    ed25519_signing_capability, verify_ed25519_signature, SignedPayload, SignedPayloadProducer,
    SignedPayloadProducerInstanceApi,
};

pub fn run() -> Result<SignedPayload, Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.ed25519-signing")?
        .with(ed25519_signing_capability("release"))
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.ed25519-signing.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let producer = instance.component::<SignedPayloadProducer>()?;
    producer.reconcile()?;
    let signed = futures::executor::block_on(producer.sign_payload(b"release payload".to_vec()))??;
    assert!(verify_ed25519_signature(
        &signed.public_key,
        &signed.payload,
        &signed.signature
    )?);

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
