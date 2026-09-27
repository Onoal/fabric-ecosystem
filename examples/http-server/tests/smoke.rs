use std::process::Command;

#[test]
fn binary_runs_http_server_example() {
    let output = Command::new(env!("CARGO_BIN_EXE_fabric-ecosystem-example-http-server"))
        .output()
        .expect("run http server example");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert!(stdout.contains("HTTP server bound to 127.0.0.1:"));
    assert!(stdout.contains("request: GET /example"));
    assert!(stdout.contains("response: 200 hello from /example"));
}
