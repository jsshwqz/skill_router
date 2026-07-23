use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

#[test]
fn bare_standard_cli_starts_acp_transport() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge"))
        .env("AI_BASE_URL", "https://acp.invalid/v1")
        .env("AI_API_KEY", "bare-acp-secret-sentinel")
        .env("AI_MODEL", "test-model")
        .env(
            "AI_PROVIDERS_DISABLED",
            "host-anthropic-proxy,opencode-zen,openrouter,openai-compatible,google-ai-compatible,zhipu-compatible,deepseek,ollama-local",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bare Forge CLI should start");

    let mut stdin = child.stdin.take().expect("stdin should be piped");
    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientCapabilities": {},
                "clientInfo": {
                    "name": "aionui-bare-cli-contract",
                    "title": "AionUI Bare CLI Contract",
                    "version": "0.1.0"
                }
            }
        })
    )
    .expect("initialize request should be written");
    drop(stdin);

    let output = child.wait_with_output().expect("bare Forge CLI should exit on EOF");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response = String::from_utf8(output.stdout)
        .expect("ACP stdout should be UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("ACP stdout must contain JSON-RPC lines"))
        .find(|message| message.get("id") == Some(&json!(1)))
        .expect("initialize response should be visible");
    assert_eq!(response["result"]["protocolVersion"], 1);
}
