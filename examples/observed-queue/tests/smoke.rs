use std::process::Command;

#[test]
fn binary_runs_observed_queue_example() {
    let output = Command::new(env!(
        "CARGO_BIN_EXE_fabric-ecosystem-example-observed-queue"
    ))
    .output()
    .expect("run observed queue example");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("first send: accepted"));
    assert!(stdout.contains("second send: full at capacity 1"));
    assert!(stdout.contains("successful sends: 1"));
    assert!(stdout.contains("failed sends: 1"));
}
