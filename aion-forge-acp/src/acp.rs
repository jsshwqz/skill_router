//! ACP (Agent Control Protocol) 服务器模式
//!
//! 通过 stdin/stdout JSON-RPC 与 AionUi 通信。
//! 将 forge 的 72 个工具通过 ACP 协议暴露给 AionUi。

use std::path::PathBuf;

use agent_client_protocol::schema::v1::{AgentCapabilities, Implementation, InitializeRequest, InitializeResponse};
use agent_client_protocol::{Agent, Client, ConnectionTo, Dispatch, Stdio, UntypedMessage};
use anyhow::Result;
use serde_json::{json, Value};

use aion_router::config::{candidate_ai_endpoints, AiEndpoint};
use aion_types::types::ExecutionContext;

/// 从 candidate_ai_endpoints() 动态构建可用模型列表
fn acp_available_models(current: &str) -> Value {
    let endpoints = candidate_ai_endpoints();
    // 只取未禁用的端点，去重（同 modelId 只保留第一个）
    let mut seen = std::collections::HashSet::new();
    let models: Vec<Value> = endpoints
        .iter()
        .filter(|ep| !AiEndpoint::is_disabled(&ep.label))
        .filter(|ep| seen.insert(ep.model.as_str()))
        .map(|ep| {
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
        })
        .collect();
    let current_id = if current.is_empty() || !endpoints.iter().any(|e| e.model == current) {
        endpoints.first().map(|e| e.model.as_str()).unwrap_or("deepseek-chat")
    } else {
        current
    };
    json!({"availableModels": models, "currentModelId": current_id})
}

fn prompt_text(params: &Value) -> String {
    if let Some(message) = params.get("message").and_then(Value::as_str) {
        return message.to_string();
    }

    params
        .get("prompt")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn visible_text(response: &Value) -> String {
    let content = response
        .pointer("/result/content")
        .and_then(Value::as_array)
        .or_else(|| response.pointer("/result/messages/0/content").and_then(Value::as_array));

    content
        .into_iter()
        .flatten()
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_message_update(raw: &str, response: &Value) -> Option<Value> {
    let request: Value = serde_json::from_str(raw).ok()?;
    if request.get("method").and_then(Value::as_str) != Some("session/prompt") {
        return None;
    }

    let session_id = request.pointer("/params/sessionId")?.as_str()?;
    let text = visible_text(response);
    if text.is_empty() {
        return None;
    }

    Some(json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": text}
            }
        }
    }))
}

/// 运行 ACP 协议服务器（stdin/stdout JSON-RPC）
pub async fn run_acp_server() -> Result<()> {
    Agent
        .builder()
        .name("aion-forge")
        .on_receive_request(
            async move |initialize: InitializeRequest, responder, _connection| {
                responder.respond(
                    InitializeResponse::new(initialize.protocol_version)
                        .agent_capabilities(AgentCapabilities::new())
                        .agent_info(Implementation::new("aion-forge", env!("CARGO_PKG_VERSION")).title("Aion Forge")),
                )
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_dispatch(
            async move |message: Dispatch, connection: ConnectionTo<Client>| {
                dispatch_legacy_message(message, connection).await
            },
            agent_client_protocol::on_receive_dispatch!(),
        )
        .connect_to(Stdio::new().with_debug(|line, direction| {
            tracing::trace!(?direction, bytes = line.len(), "ACP transport line");
        }))
        .await
        .map_err(anyhow::Error::new)
}

async fn dispatch_legacy_message(
    message: Dispatch,
    connection: ConnectionTo<Client>,
) -> agent_client_protocol::Result<()> {
    match message {
        Dispatch::Request(message, responder) => {
            let raw = json!({
                "jsonrpc": "2.0",
                "id": responder.id(),
                "method": message.method,
                "params": message.params,
            })
            .to_string();

            match handle_acp_message(&raw).await {
                Ok(response) => {
                    if let Some(update) = agent_message_update(&raw, &response) {
                        send_legacy_notification(&connection, update)?;
                    }

                    if let Some(result) = response.get("result") {
                        responder.respond(result.clone())
                    } else if let Some(error) = response.get("error") {
                        responder.respond_with_error(legacy_error(error))
                    } else {
                        responder.respond_with_internal_error("legacy ACP handler returned no result")
                    }
                }
                Err(error) => {
                    responder.respond_with_error(agent_client_protocol::Error::internal_error().data(error.to_string()))
                }
            }
        }
        Dispatch::Notification(message) => {
            let raw = json!({
                "jsonrpc": "2.0",
                "method": message.method,
                "params": message.params,
            })
            .to_string();
            if let Err(error) = handle_acp_message(&raw).await {
                tracing::warn!(%error, "legacy ACP notification failed");
            }
            Ok(())
        }
        Dispatch::Response(_, _) => Ok(()),
    }
}

fn send_legacy_notification(
    connection: &ConnectionTo<Client>,
    notification: Value,
) -> agent_client_protocol::Result<()> {
    let method = notification
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(agent_client_protocol::Error::invalid_request)?;
    let params = notification.get("params").cloned().unwrap_or(Value::Null);
    connection.send_notification(UntypedMessage::new(method, params)?)
}

fn legacy_error(error: &Value) -> agent_client_protocol::Error {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .and_then(|code| i32::try_from(code).ok())
        .unwrap_or(-32603);
    let message = error.get("message").and_then(Value::as_str).unwrap_or("Internal error");
    let mut mapped = agent_client_protocol::Error::new(code, message);
    if let Some(data) = error.get("data") {
        mapped = mapped.data(data.clone());
    }
    mapped
}

async fn handle_acp_message(raw: &str) -> Result<Value> {
    let msg: Value = serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("Invalid JSON-RPC: {}", e))?;

    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg["method"].as_str().unwrap_or("").to_string();
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    match method.as_str() {
        "session/new" => {
            let session_id = format!(
                "forge_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
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
        "session/update" | "session/delete" => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": null
        })),

        "session/prompt" => {
            let message = prompt_text(&params);
            let model = params.get("model").and_then(|v| v.as_str()).unwrap_or("deepseek-chat");
            let history = params.get("history").and_then(|v| v.as_array());

            // 构建发给 ai_task 的上下文
            let mut ctx = ExecutionContext::new("ai_task", &message).with_context(json!({
                "model": model,
                "input": &message,
                "text": &message,
                "stream": false,
            }));

            // 如果有历史消息，加入上下文
            if let Some(msgs) = history {
                ctx = ctx.with_context(json!({"history": msgs}));
            }

            let registry = aion_router::builtins::BuiltinRegistry::default_registry();
            if let Some(builtin) = registry.get("ai_task") {
                let skill = aion_types::types::SkillDefinition {
                    metadata: aion_types::types::SkillMetadata {
                        name: "ai_task".to_string(),
                        version: "0.1.0".to_string(),
                        capabilities: vec!["ai_task".to_string()],
                        entrypoint: "builtin:ai_task".to_string(),
                        permissions: aion_types::types::PermissionSet::default_deny().with_network(true),
                        instruction: Some("你是一个AI助手，请根据用户输入给出回答。".to_string()),
                        engine_capable: false,
                    },
                    root_dir: PathBuf::new(),
                    source: aion_types::types::SkillSource::Local,
                };

                match builtin.execute(&skill, &ctx).await {
                    Ok(result) => {
                        let content = result.get("output").and_then(|c| c.as_str()).unwrap_or("");
                        let provider = result.get("provider").and_then(|p| p.as_str()).unwrap_or("unknown");
                        let input_tokens = result
                            .get("token_usage")
                            .and_then(|u| u.get("prompt_tokens"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let output_tokens = result
                            .get("token_usage")
                            .and_then(|u| u.get("completion_tokens"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let has_error = result.get("error").and_then(|e| e.as_str()).filter(|e| !e.is_empty());

                        if let Some(err) = has_error {
                            let mut content_parts =
                                vec![json!({"type": "text", "text": format!("[{} 调用失败: {}]", provider, err)})];
                            if !content.is_empty() {
                                content_parts.push(json!({"type": "text", "text": content}));
                            }
                            Ok(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": content_parts,
                                    "stopReason": "end_turn",
                                    "usage": {
                                        "inputTokens": input_tokens,
                                        "outputTokens": output_tokens
                                    }
                                }
                            }))
                        } else {
                            Ok(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [
                                        {"type": "text", "text": content}
                                    ],
                                    "stopReason": "end_turn",
                                    "usage": {
                                        "inputTokens": input_tokens,
                                        "outputTokens": output_tokens
                                    }
                                }
                            }))
                        }
                    }
                    Err(e) => Ok(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "messages": [{
                                "role": "assistant",
                                "content": [{"type": "text", "text": format!("[模型调用错误: {}]", e)}]
                            }],
                            "stopReason": "end_turn",
                            "usage": {"inputTokens": 0, "outputTokens": 0}
                        }
                    })),
                }
            } else {
                // fallback: ai_task builtin 未注册
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{"type": "text", "text": "[ai_task builtin not available — 请检查 builtin 注册]"}],
                        "stopReason": "end_turn",
                        "usage": {"inputTokens": 0, "outputTokens": 0}
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

        "shutdown" | "exit" => Ok(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": null
        })),

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

#[cfg(test)]
mod tests {
    use super::{agent_message_update, prompt_text};
    use serde_json::json;

    #[test]
    fn reads_standard_acp_prompt_content_blocks() {
        let params = json!({
            "prompt": [
                {"type": "text", "text": "第一段"},
                {"type": "image", "data": "ignored"},
                {"type": "text", "text": "第二段"}
            ]
        });

        assert_eq!(prompt_text(&params), "第一段\n第二段");
    }

    #[test]
    fn converts_prompt_result_to_visible_agent_message_update() {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "session/prompt",
            "params": {"sessionId": "forge_test", "prompt": []}
        })
        .to_string();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": {
                "content": [{"type": "text", "text": "可见回答"}],
                "stopReason": "end_turn"
            }
        });

        let update = agent_message_update(&request, &response).expect("visible update");
        assert_eq!(update["method"], "session/update");
        assert_eq!(update["params"]["sessionId"], "forge_test");
        assert_eq!(update["params"]["update"]["sessionUpdate"], "agent_message_chunk");
        assert_eq!(update["params"]["update"]["content"]["text"], "可见回答");
    }
}
