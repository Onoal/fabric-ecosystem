use std::process::Command;

#[test]
fn binary_runs_ed25519_signing_example() {
    let output = Command::new(env!(
        "CARGO_BIN_EXE_fabric-ecosystem-example-ed25519-signing"
    ))
    .output()
    .expect("run ed25519 signing example");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("signed payload: release payload"));
    assert!(stdout.contains("signature verified: true"));
    assert!(stdout.contains("public key bytes: 32"));
}
