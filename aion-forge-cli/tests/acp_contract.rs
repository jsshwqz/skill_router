use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn canonical_cli_runs_acp_with_json_only_stdout() {
    let cwd = serde_json::to_string(&std::env::current_dir().expect("test cwd should exist"))
        .expect("test cwd should serialize");
    let input = format!(
        concat!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":1,"clientCapabilities":{{}},"clientInfo":{{"name":"aion-forge-test","title":"Aion Forge Test","version":"0.1.0"}}}}}}"#,
            "\n",
            r#"{{"jsonrpc":"2.0","id":2,"method":"session/new","params":{{"cwd":{},"mcpServers":[]}}}}"#,
            "\n"
        ),
        cwd
    );
    let responses = run_acp(&input);

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["protocolVersion"], 1);
    assert!(responses[0]["result"]["agentInfo"]["name"].is_string());
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"]["configOptions"][0]["id"], "model");
    assert!(responses[1]["result"]["configOptions"][0]["currentValue"].is_string());
}

#[test]
fn canonical_cli_rejects_initialize_without_required_acp_fields() {
    let responses = run_acp(concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#,
        "\n"
    ));

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 2);
    assert_eq!(responses[0]["error"]["code"], -32602);
}

fn run_acp(input: &str) -> Vec<serde_json::Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge"))
        .arg("acp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("canonical Forge CLI should start");

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
    responses
}
