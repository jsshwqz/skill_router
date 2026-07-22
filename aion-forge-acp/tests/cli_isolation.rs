use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aion-forge-acp"))
        .args(args)
        .output()
        .expect("aion-forge-acp should start")
}

fn assert_rejected(args: &[&str]) {
    let output = run(args);
    assert!(
        !output.status.success(),
        "legacy CLI argument {args:?} should be rejected"
    );
}

#[test]
fn acp_subcommand_starts_protocol_server_without_stdout_contamination() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge-acp"))
        .arg("acp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ACP adapter should start");

    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}"#,
        "\n"
    );
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("ACP requests should be written");

    let output = child.wait_with_output().expect("ACP adapter should exit");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let responses: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout must contain only JSON-RPC lines"))
        .collect();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["protocolVersion"], 1);
    assert_eq!(responses[1]["id"], 2);
}

#[test]
fn legacy_direct_execution_flags_are_rejected() {
    assert_rejected(&["--tool", "ai_task"]);
    assert_rejected(&["--params", "{}"]);
    assert_rejected(&["--list"]);
}

#[test]
fn non_acp_subcommands_are_rejected() {
    assert_rejected(&["setup"]);
    assert_rejected(&["mcp-server"]);
}

#[test]
fn help_describes_only_the_acp_adapter() {
    let output = run(&["--help"]);
    assert!(output.status.success());

    let help = String::from_utf8(output.stdout).expect("help should be UTF-8");
    assert!(help.contains("ACP"));
    for legacy in ["--tool", "--params", "--list", "setup", "mcp-server", "直接工具入口"] {
        assert!(!help.contains(legacy), "help exposed legacy surface: {legacy}");
    }
}
