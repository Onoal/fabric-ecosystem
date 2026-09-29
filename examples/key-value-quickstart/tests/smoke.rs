use std::process::Command;

#[test]
fn binary_runs_key_value_quickstart() {
    let output = Command::new(env!(
        "CARGO_BIN_EXE_fabric-ecosystem-example-key-value-quickstart"
    ))
    .output()
    .expect("run key-value quickstart example");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("stored: fabric"));
    assert!(stdout.contains("after delete: missing"));
}
