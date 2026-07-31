use std::collections::HashSet;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

#[test]
fn initialize_uses_forge_product_identity() {
    let response = aion_forge_cli::mcp::initialize_response(json!(7));
    assert_eq!(response["id"], 7);
    assert_eq!(response["result"]["serverInfo"]["name"], "aion-forge");
    assert_eq!(response["result"]["serverInfo"]["version"], "0.7.0");
}

#[test]
fn tools_list_matches_direct_catalog_without_duplicates() {
    let response = aion_forge_cli::mcp::tools_list_response(json!(8));
    let tools = response["result"]["tools"].as_array().expect("tools must be an array");
    assert!(!tools.is_empty(), "tools catalog must not be empty");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name must be a string"))
        .collect();
    let unique: HashSet<&str> = names.iter().copied().collect();
    let direct = aion_forge_cli::direct::list_tools();
    let direct_names: HashSet<&str> = direct["tools"]
        .as_array()
        .expect("direct tools must be an array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("direct tool name must be a string"))
        .collect();

    assert_eq!(unique.len(), tools.len(), "MCP catalog contains duplicate names");
    assert_eq!(
        unique, direct_names,
        "direct and MCP catalogs must expose the same names"
    );
    assert!(
        tools.iter().all(|tool| tool["inputSchema"].is_object()),
        "every MCP tool must define an object inputSchema"
    );
}

#[test]
fn every_protocol_output_line_is_json_rpc() {
    let mut output = Vec::new();
    aion_forge_cli::mcp::write_json_line(&mut output, &aion_forge_cli::mcp::initialize_response(json!(9)))
        .expect("response should serialize");

    let text = String::from_utf8(output).expect("stdout should be UTF-8");
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).expect("stdout line must be JSON");
        assert_eq!(value["jsonrpc"], "2.0");
    }
}

#[test]
fn tools_call_routes_the_sanitize_catalog_entry() {
    let workspace = std::env::temp_dir().join(format!("aion-forge-cli-sanitize-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("isolated workspace must be created");
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge-cli"))
        .arg("mcp-server")
        .current_dir(&workspace)
        .env("AI_PASSTHROUGH", "false")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Forge MCP must start");
    let mut stdin = child.stdin.take().expect("stdin must be piped");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{{"name":"sanitize","arguments":{{"text":"safe contract"}}}}}}"#
    )
    .expect("sanitize request must be written");
    drop(stdin);

    let output = child.wait_with_output().expect("Forge MCP must exit on EOF");
    std::fs::remove_dir_all(&workspace).expect("isolated workspace must be removed");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let response: Value = serde_json::from_str(stdout.trim()).expect("stdout must contain one JSON-RPC response");
    assert_eq!(response["result"]["isError"], false, "response: {response}");
    let content: Value = serde_json::from_str(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("tool content must be text"),
    )
    .expect("tool content must contain JSON");
    assert_eq!(content["clean_text"], "safe contract", "response: {response}");
    assert_eq!(content["sanitized_count"], 0);
}
