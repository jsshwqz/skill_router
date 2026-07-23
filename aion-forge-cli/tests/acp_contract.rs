use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn canonical_cli_runs_acp_with_json_only_stdout() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge"))
        .arg("acp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("canonical Forge CLI should start");

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

    let output = child.wait_with_output().expect("ACP server should exit");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let responses: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout must contain only JSON-RPC lines"))
        .collect();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[1]["id"], 2);
}
