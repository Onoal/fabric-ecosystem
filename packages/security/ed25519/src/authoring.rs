use fabric::prelude::*;

use crate::{Ed25519Signer, EphemeralEd25519Signer};

pub fn ephemeral_ed25519_signer(name: &'static str) -> impl IntoFabricContribution {
    let selected = Ed25519Signer::select(name).expect("valid Ed25519 signer resource name");
    FabricContribution::new().resource(
        selected
            .using(EphemeralEd25519Signer::new())
            .expect("EphemeralEd25519Signer supports Ed25519Signer"),
    )
}
