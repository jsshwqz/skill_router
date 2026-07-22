use serde_json::{json, Value};

#[test]
fn initialize_uses_forge_product_identity() {
    let response = aion_forge_cli::mcp::initialize_response(json!(7));
    assert_eq!(response["id"], 7);
    assert_eq!(response["result"]["serverInfo"]["name"], "aion-forge");
    assert_eq!(response["result"]["serverInfo"]["version"], "0.7.0");
}

#[test]
fn tools_list_exposes_current_75_tools() {
    let response = aion_forge_cli::mcp::tools_list_response(json!(8));
    let tools = response["result"]["tools"].as_array().expect("tools must be an array");
    assert_eq!(tools.len(), 75);
    assert!(tools.iter().all(|tool| tool["inputSchema"].is_object()));
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
