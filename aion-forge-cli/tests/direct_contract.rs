use std::process::Command;

#[tokio::test]
async fn execute_runs_echo_from_builtin_registry() {
    let result = aion_forge_cli::direct::execute("echo", Some(r#"{"text":"forge-direct-contract"}"#), true)
        .await
        .expect("echo should execute through the built-in registry");

    assert_eq!(result["echo"], "forge-direct-contract");
    assert_eq!(result["capability"], "echo");
    assert_eq!(result["length"], 21);
}

#[test]
fn quiet_direct_output_is_one_compact_json_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_aion-forge-cli"))
        .args(["--tool", "echo", "--params", r#"{"text":"quiet-contract"}"#, "--quiet"])
        .output()
        .expect("quiet direct invocation should start");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(stdout.lines().count(), 1, "quiet output must be one JSON line");
    assert_eq!(
        stdout.trim_end(),
        r#"{"capability":"echo","echo":"quiet-contract","length":14}"#
    );
}
