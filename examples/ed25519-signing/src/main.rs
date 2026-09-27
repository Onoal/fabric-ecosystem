mod app;

use std::io::Error;

use app::{ExampleSigningApp, ExampleSigningAppInstanceApi};
use fabric::prelude::*;
use fabric_package_security_ed25519::{ephemeral_ed25519_signer, verify_ed25519_signature};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payload = b"release payload".to_vec();
    let composition = Fabric::new("fabric.ecosystem.example.ed25519-signing")?
        .with(ephemeral_ed25519_signer("release"))
        .with(app::signing_app("release"))
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.ed25519-signing.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let app = instance.component::<ExampleSigningApp>()?;
    app.reconcile()?;
    let signed = futures::executor::block_on(app.sign_payload(payload))??;
    let verified = verify_ed25519_signature(&signed.public_key, &signed.payload, &signed.signature);
    if !verified {
        return Err(Box::new(Error::other("signature verification failed")));
    }

    instance.stop()?;

    println!(
        "signed payload: {}",
        String::from_utf8_lossy(&signed.payload)
    );
    println!("signature verified: true");
    println!("public key bytes: {}", signed.public_key.as_bytes().len());

    Ok(())
}
