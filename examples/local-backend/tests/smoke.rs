use std::process::Command;

#[test]
fn binary_runs_local_backend_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_fabric-ecosystem-example-local-backend"))
        .output()
        .expect("run local backend example");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("database value: local-data"));
    assert!(stdout.contains("log target: local-backend-example"));
    assert!(stdout.contains("HTTP server bound to 127.0.0.1:"));
    assert!(stdout.contains("request: GET /local-backend"));
    assert!(stdout.contains("response: 200 local-data via /local-backend"));
}
