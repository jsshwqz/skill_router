//! ACP (Agent Control Protocol) 服务器模式
//!
//! 通过 stdin/stdout JSON-RPC 与 AionUi 通信。
//! 将 forge 的 72 个工具通过 ACP 协议暴露给 AionUi。

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{json, Value};

use aion_router::config::candidate_ai_endpoints;
use aion_types::types::ExecutionContext;

/// 从 candidate_ai_endpoints() 动态构建可用模型列表
fn acp_available_models(current: &str) -> Value {
    let endpoints = candidate_ai_endpoints();
    let models: Vec<Value> = endpoints.iter().map(|ep| {
        let label = match ep.label.as_str() {
            "deepseek" => format!("DeepSeek ({})", ep.model),
            "primary" => format!("主力 ({})", ep.model),
            "openrouter" => format!("OpenRouter ({})", ep.model),
            "openai-compatible" => format!("GPT兼容 ({})", ep.model),
            "google-ai-compatible" => format!("GitCode 免费 ({})", ep.model),
            "zhipu-compatible" => format!("智谱 GLM 免费 ({})", ep.model),
            "opencode-zen" => format!("OpenCode ({})", ep.model),
            "ollama-local" => format!("Ollama 本地 ({})", ep.model),
            other => format!("{} ({})", other, ep.model),
        };
        json!({"modelId": ep.model, "name": label})
    }).collect();
    let current_id = if current.is_empty() || !endpoints.iter().any(|e| e.model == current) {
        endpoints.first().map(|e| e.model.as_str()).unwrap_or("deepseek-chat")
    } else { current };
    json!({"availableModels": models, "currentModelId": current_id})
}

/// 从 stdin 读取一条完整消息
/// 支持两种帧格式：
///   1. Content-Length: N\r\n\r\n{json}   — 标准 MCP/ACP 格式
///   2. {json}\n                           — 纯 JSON 行格式
fn read_message<R: BufRead>(reader: &mut R) -> Result<String> {
    let mut line = String::new();
    line.clear();
    if reader.read_line(&mut line)? == 0 {
        return Err(anyhow::anyhow!("EOF"));
    }
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    // 检测 Content-Length 帧格式
    if trimmed.to_lowercase().starts_with("content-length:") {
        let len_str = trimmed
            .split(':')
            .nth(1)
            .unwrap_or("")
            .trim();
        let content_len: usize = len_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid Content-Length value '{}': {}", len_str, e))?;

        // 跳过空白行（\r\n\r\n 的第二部分）
        let mut blank = String::new();
        reader.read_line(&mut blank)?;

        // 读取 JSON 体
        let mut buf = vec![0u8; content_len];
        reader.read_exact(&mut buf)?;
        let body = String::from_utf8(buf)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 body: {}", e))?;

        Ok(body)
    } else {
        // 纯 JSON 行格式
        Ok(trimmed)
    }
}

/// 运行 ACP 协议服务器（stdin/stdout JSON-RPC）
pub async fn run_acp_server() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();

    loop {
        let raw = match read_message(&mut reader) {
            Ok(s) => s,
            Err(e) => {
                // EOF 正常退出
                if e.to_string() == "EOF" {
                    break;
                }
                eprintln!("ACP read error: {}", e);
                break;
            }
        };

        if raw.is_empty() {
            continue;
        }

        // 对于 aioncore，输出永远用纯 JSON 行（行尾加 \n），
        // 因为 aioncore 的 MCP/ACP 客户端用 reader.read_line() 解析响应，
        // Content-Length 帧会让 aioncore 读到 "Content-Length:" 作为一行 → JSON 解析失败
        let send_line = |json: &str| -> Result<()> {
            let mut out = stdout.lock();
            writeln!(out, "{}", json)?;
            out.flush()?;
            Ok(())
        };

        match handle_acp_message(&raw).await {
            Ok(response) => {
                let json = serde_json::to_string(&response)?;
                send_line(&json)?;

                if is_shutdown(&raw) {
                    break;
                }
            }
            Err(e) => {
                let error_resp = json!({
                    "jsonrpc": "2.0",
                    "id": extract_id(&raw),
                    "error": {
                        "code": -32603,
                        "message": format!("Internal error: {}", e)
                    }
                });
                let json = serde_json::to_string(&error_resp)?;
                send_line(&json)?;
            }
        }
    }

    Ok(())
}

async fn handle_acp_message(raw: &str) -> Result<Value> {
    let msg: Value = serde_json::from_str(raw)
        .map_err(|e| anyhow::anyhow!("Invalid JSON-RPC: {}", e))?;

    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg["method"].as_str().unwrap_or("").to_string();
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    match method.as_str() {
        "initialize" => {
            // 记录收到的 initialize 参数（用于调试）
            eprintln!("[forge-acp] >>>>>> initialize params: {}", serde_json::to_string_pretty(&params).unwrap_or_default());
            
            // 检测客户端类型：MCP 客户端发送 protocolVersion，ACP 客户端可能没有
            let protocol_ver = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("");
            let is_mcp = protocol_ver.starts_with("2024") || protocol_ver.starts_with("2025") || protocol_ver.starts_with("0.1");

            // 可用模型列表（从配置读取）
            let model = std::env::var("AI_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".to_string());
            let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

            if is_mcp {
                // MCP 格式（兼容 Claude Code / Reasonix / AionUI v0.1.33+）
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2025-01-17",
                        "capabilities": {
                            "tools": {},
                            "prompts": {},
                            "resources": {},
                            "session": {}
                        },
                        "serverInfo": {
                            "name": "aion-forge",
                            "version": "0.7.0"
                        },
                        "availableModels": acp_available_models(&model).get("availableModels").unwrap_or(&Value::Null).as_array().cloned().unwrap_or_default(),
                        "currentModelId": model
                    }
                }))
            } else {
                // ACP 格式（兼容 AionUI v0.1.44+）
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": 1,
                        "agentCapabilities": {
                            "loadSession": true,
                            "mcpCapabilities": { "http": false, "sse": false },
                            "promptCapabilities": { "audio": false, "embeddedContext": false, "image": false },
                            "sessionCapabilities": { "delete": {}, "list": {} }
                        },
                        "agentInfo": { "name": "aion-forge", "title": "Aion Forge", "version": "0.7.0" },
                        "authMethods": [],
                        "availableModels": acp_available_models(&model).get("availableModels").unwrap_or(&Value::Null).as_array().cloned().unwrap_or_default(),
                        "currentModelId": model
                    }
                }))
            }
        }

        "session/new" => {
            eprintln!("[forge-acp] session/new params: {}", serde_json::to_string(&params).unwrap_or_default());
            let session_id = format!("forge_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "sessionId": session_id,
                    "models": acp_available_models("deepseek-v4-flash"),
                    "modes": {
                        "availableModes": [
                            {"id":"chat","name":"Chat"},
                            {"id":"reasoning","name":"Reasoning"}
                        ],
                        "currentModeId": "chat"
                    },
                    "configOptions": []
                }
            }))
        }
        "session/update" | "session/delete" => {
            eprintln!("[forge-acp] {} params: {}", method, serde_json::to_string(&params).unwrap_or_default());
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
            }))
        }

        "session/prompt" => {
            eprintln!("[forge-acp] session/prompt params: {}", serde_json::to_string(&params).unwrap_or_default());
            // ACP PromptResponse schema: { stopReason: "end_turn"|"max_tokens"|"stop_sequence", usage?: {...} }
            // Forge acts as a tool provider; minimal valid response to avoid -32601
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "stopReason": "end_turn",
                    "usage": {
                        "inputTokens": 0,
                        "outputTokens": 0
                    }
                }
            }))
        }

        "tools/list" | "mcp/toolsList" | "listTools" => {
            let caps = aion_types::capability_registry::CapabilityRegistry::builtin();
            let tools: Vec<Value> = caps.definitions().map(|d| {
                let mut schema = d.parameters_schema.clone();
                if !schema.is_object() || schema.get("type").is_none() {
                    schema = json!({"type": "object", "properties": {}, "required": []});
                }
                json!({
                    "name": d.name,
                    "description": d.description,
                    "inputSchema": schema
                })
            }).collect();

            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tools }
            }))
        }

        "tools/call" | "mcp/toolsCall" | "callTool" => {
            let tool_name = params["name"].as_str()
                .or_else(|| params["tool"].as_str())
                .unwrap_or("")
                .to_string();

            let arguments = params.get("arguments")
                .or_else(|| params.get("params"))
                .cloned()
                .unwrap_or(json!({}));

            let registry = aion_router::builtins::BuiltinRegistry::default_registry();

            if let Some(builtin) = registry.get(&tool_name) {
                let ctx = ExecutionContext::new(&tool_name, &tool_name)
                    .with_context(arguments);

                let skill = aion_types::types::SkillDefinition {
                    metadata: aion_types::types::SkillMetadata {
                        name: tool_name.clone(),
                        version: "0.1.0".to_string(),
                        capabilities: vec![tool_name.clone()],
                        entrypoint: format!("builtin:{}", tool_name),
                        permissions: aion_types::types::PermissionSet::default_deny().with_network(true),
                        instruction: None,
                        engine_capable: false,
                    },
                    root_dir: PathBuf::new(),
                    source: aion_types::types::SkillSource::Local,
                };

                match builtin.execute(&skill, &ctx).await {
                    Ok(result) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&result)?
                            }],
                            "isError": false
                        }
                    })),
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }],
                            "isError": true
                        }
                    })),
                }
            } else {
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32602,
                        "message": format!("Unknown tool: {}", tool_name)
                    }
                }))
            }
        }

        "notifications/initialized" | "initialized" => {
            // 忽略
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
            }))
        }

        "shutdown" | "exit" => {
            Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
            }))
        }

        _ => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("Method not found: {}", method)
            }
        })),
    }
}

fn extract_id(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(Value::Null)
}

fn is_shutdown(raw: &str) -> bool {
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        v.get("method")
            .and_then(|m| m.as_str())
            .map(|m| m == "shutdown" || m == "exit")
            .unwrap_or(false)
    } else {
        false
    }
}
