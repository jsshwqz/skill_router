use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
};

use serde_json::{json, Value};

#[test]
fn stateful_acp_session_exposes_models_and_handles_bootstrap_without_leaks() {
    let cwd = std::env::current_dir().expect("test working directory should exist");
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge-acp"))
        .arg("acp")
        .env("AI_BASE_URL", "https://acp.invalid/v1")
        .env("AI_API_KEY", "acp-secret-sentinel")
        .env("AI_MODEL", "test-model")
        .env(
            "AI_PROVIDERS_DISABLED",
            "host-anthropic-proxy,opencode-zen,openrouter,openai-compatible,google-ai-compatible,zhipu-compatible,deepseek,ollama-local",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ACP adapter should start");

    let mut stdin = child.stdin.take().expect("stdin should be piped");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout should be piped"));
    let mut stderr = child.stderr.take().expect("stderr should be piped");
    let mut wire = Vec::new();

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientCapabilities": {},
                "clientInfo": {
                    "name": "aion-forge-acp-contract",
                    "title": "Aion Forge ACP Contract",
                    "version": "0.1.0"
                }
            }
        }),
    );
    let initialize = read_response(&mut stdout, 1, &mut wire);

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "session/new",
            "params": {"cwd": cwd, "mcpServers": []}
        }),
    );
    let new_session = read_response(&mut stdout, 2, &mut wire);
    let session_id = new_session["result"]["sessionId"]
        .as_str()
        .expect("session/new should return a session ID")
        .to_string();

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "session/set_config_option",
            "params": {"sessionId": session_id, "configId": "model", "value": "test-model"}
        }),
    );
    let set_model = read_response(&mut stdout, 3, &mut wire);

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "session/prompt",
            "params": {
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": "[Skill: aion-forge]\nUse Forge capabilities."}]
            }
        }),
    );
    let bootstrap = read_response(&mut stdout, 4, &mut wire);

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "session/prompt",
            "params": {
                "sessionId": session_id,
                "modelId": "missing-model",
                "prompt": [{"type": "text", "text": "hello"}]
            }
        }),
    );
    let invalid_model = read_response(&mut stdout, 5, &mut wire);

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "session/prompt",
            "params": {
                "sessionId": "missing-session",
                "prompt": [{"type": "text", "text": "hello"}]
            }
        }),
    );
    let unknown = read_response(&mut stdout, 6, &mut wire);

    send_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "session/not_supported",
            "params": {}
        }),
    );
    let unsupported = read_response(&mut stdout, 7, &mut wire);
    drop(stdin);

    let mut remaining_stdout = String::new();
    stdout
        .read_to_string(&mut remaining_stdout)
        .expect("remaining stdout should be readable");
    let mut stderr_text = String::new();
    stderr
        .read_to_string(&mut stderr_text)
        .expect("stderr should be readable");
    let status = child.wait().expect("ACP adapter should exit");
    assert!(status.success(), "stderr: {stderr_text}");

    assert_eq!(initialize["result"]["protocolVersion"], 1);
    assert_eq!(new_session["result"]["configOptions"][0]["id"], "model");
    assert_eq!(new_session["result"]["configOptions"][0]["currentValue"], "test-model");
    assert_eq!(set_model["result"]["configOptions"][0]["currentValue"], "test-model");
    assert_eq!(bootstrap["result"]["stopReason"], "end_turn");
    assert_eq!(invalid_model["result"]["stopReason"], "end_turn");
    let invalid_model_message = wire
        .iter()
        .find(|message| {
            message.pointer("/params/update/sessionUpdate").and_then(Value::as_str) == Some("agent_message_chunk")
                && message
                    .pointer("/params/update/content/text")
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.contains("missing-model"))
        })
        .expect("invalid prompt model should produce a visible message");
    assert!(invalid_model_message["params"]["update"]["content"]["text"]
        .as_str()
        .expect("visible invalid-model message should be text")
        .contains("test-model"));
    assert!(unknown["error"]["message"]
        .as_str()
        .expect("unknown-session error should have a message")
        .contains("unknown ACP session"));
    assert_eq!(unknown["error"]["code"], -32602);
    assert_eq!(unsupported["error"]["code"], -32601);

    let stdout_text = wire.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    assert!(remaining_stdout.trim().is_empty());
    assert!(!stdout_text.contains("acp-secret-sentinel"));
    assert!(!stderr_text.contains("acp-secret-sentinel"));
}

fn send_request(stdin: &mut impl Write, request: Value) {
    writeln!(stdin, "{request}").expect("ACP request should be written");
    stdin.flush().expect("ACP request should be flushed");
}

fn read_response(stdout: &mut impl BufRead, id: i64, wire: &mut Vec<Value>) -> Value {
    loop {
        let mut line = String::new();
        let bytes = stdout.read_line(&mut line).expect("ACP stdout should be readable");
        assert_ne!(bytes, 0, "ACP process ended before response {id}");
        let message: Value = serde_json::from_str(line.trim()).expect("stdout must contain only JSON-RPC lines");
        wire.push(message.clone());
        if message.get("id").and_then(Value::as_i64) == Some(id) {
            return message;
        }
    }
}
