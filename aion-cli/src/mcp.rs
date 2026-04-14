//! MCP (Model Context Protocol) stdio server
//!
//! 实现 JSON-RPC over stdin/stdout，让 Claude Desktop 等 MCP 客户端
//! 可以直接调用 Aion Forge 的所有能力。
//!
//! ## 支持的 MCP 方法
//! - `initialize` → 返回服务器信息和能力列表
//! - `tools/list` → 从 CapabilityRegistry 生成 tool 定义
//! - `tools/call` → 映射为 SkillRouter 调用

use std::io::{self, BufRead, Write};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

use aion_router::SkillRouter;
use aion_types::types::RouterPaths;

/// 异步任务最大等待时间（秒）
const ASYNC_POLL_TIMEOUT_SECS: u64 = 300;
/// 轮询间隔（秒）
const ASYNC_POLL_INTERVAL_SECS: u64 = 5;

/// JSON-RPC 2.0 请求
#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC 2.0 响应
#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

/// MCP 通知（无 id，无需响应）
#[allow(dead_code)]
#[derive(Serialize)]
struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// 运行 MCP stdio server
pub async fn run_mcp_server(paths: RouterPaths) -> Result<()> {
    // 初始化学习引擎
    aion_router::learner::init_learner(&paths.workspace_root);

    let router = SkillRouter::new(paths)?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    info!("MCP server started, reading from stdin...");

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    Value::Null,
                    -32700,
                    &format!("Parse error: {}", e),
                );
                write_response(&mut stdout_lock, &resp)?;
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            let resp = JsonRpcResponse::error(
                request.id.unwrap_or(Value::Null),
                -32600,
                "Invalid JSON-RPC version",
            );
            write_response(&mut stdout_lock, &resp)?;
            continue;
        }

        // 通知（method 以 "notifications/" 开头，或 "initialized"）没有 id，不应回复
        if request.id.is_none() || request.method.starts_with("notifications/") || request.method == "initialized" {
            // MCP 通知：静默处理，不返回任何响应
            continue;
        }

        let id = request.id.unwrap_or(Value::Null);

        let response = match request.method.as_str() {
            "initialize" => handle_initialize(id),
            "tools/list" => handle_tools_list(id, &router),
            "tools/call" => handle_tools_call(id, &request.params, &router).await,
            _ => JsonRpcResponse::error(id, -32601, &format!("Method not found: {}", request.method)),
        };

        write_response(&mut stdout_lock, &response)?;
    }

    Ok(())
}

fn write_response(out: &mut impl Write, resp: &JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(resp)?;
    writeln!(out, "{}", json)?;
    out.flush()?;
    Ok(())
}

/// 处理 `initialize` — 返回服务器信息
fn handle_initialize(id: Value) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "aion-forge",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

/// 处理 `tools/list` — 从 CapabilityRegistry 生成 MCP tool 列表
fn handle_tools_list(id: Value, router: &SkillRouter) -> JsonRpcResponse {
    let mut tools: Vec<Value> = router
        .registry()
        .definitions()
        .map(|cap| {
            json!({
                "name": cap.name,
                "description": cap.description,
                "inputSchema": if cap.parameters_schema.is_null() || cap.parameters_schema == json!({}) {
                    json!({
                        "type": "object",
                        "properties": build_properties_from_inputs(&cap.inputs),
                        "required": cap.inputs
                    })
                } else {
                    cap.parameters_schema.clone()
                }
            })
        })
        .collect();

    // 注入 async_task_query 工具——让 MCP 客户端也可以手动查询异步任务
    tools.push(json!({
        "name": "async_task_query",
        "description": "查询异步编排任务的状态和结果。不传 task_id 则列出所有任务。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "任务 ID（如 orch_69cdfa9f_0000），留空则列出所有任务"
                }
            }
        }
    }));

    JsonRpcResponse::success(id, json!({ "tools": tools }))
}

/// 检查是否启用 AI passthrough 模式（宿主 LLM 直接处理 AI 任务）
fn is_passthrough_enabled() -> bool {
    std::env::var("AI_PASSTHROUGH")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// 处理 `tools/call` — 执行技能
async fn handle_tools_call(id: Value, params: &Value, router: &SkillRouter) -> JsonRpcResponse {
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    if tool_name.is_empty() {
        return JsonRpcResponse::error(id, -32602, "Missing 'name' in tools/call params");
    }

    // === AI Passthrough 模式 ===
    // 当启用时，AI 类能力不调用外部 API，而是返回 instruction + input
    // 让宿主 LLM（Claude/Qwen 等）直接处理，质量更高且零额外 API 成本
    if is_passthrough_enabled() {
        if let Some(instruction) = aion_intel::synth::ai_instruction_for(tool_name) {
            let text = arguments.get("text")
                .or(arguments.get("input"))
                .or(arguments.get("query"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let prompt = format!(
                "[Instruction]: {}\n\n[Input]:\n{}",
                instruction, text
            );
            return JsonRpcResponse::success(id, json!({
                "content": [{"type": "text", "text": prompt}],
                "isError": false
            }));
        }
    }

    // Build a human-readable task description for logging/context
    let task = if let Some(text) = arguments.get("text").and_then(|v| v.as_str()) {
        format!("{}: {}", tool_name, text)
    } else if let Some(query) = arguments.get("query").and_then(|v| v.as_str()) {
        format!("{}: {}", tool_name, query)
    } else {
        format!("{}: {}", tool_name, serde_json::to_string(&arguments).unwrap_or_default())
    };

    // Use route_with_capability to directly target the capability by name,
    // instead of route_with_context which tries to infer capability from
    // natural language and often fails (the tool_name IS the capability).
    match router.route_with_capability(&task, tool_name, Some(arguments)).await {
        Ok(result) => {
            if result.execution.status == "ok" {
                // 检查是否为异步结果，如果是则自动等待完成
                let final_result = maybe_await_async_result(
                    &result.execution.result,
                    router,
                ).await;

                let content = json!([{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&final_result)
                        .unwrap_or_else(|_| final_result.to_string())
                }]);

                JsonRpcResponse::success(
                    id,
                    json!({
                        "content": content,
                        "isError": false
                    }),
                )
            } else {
                let content = json!([{
                    "type": "text",
                    "text": format!("Error: {}", result.execution.error.unwrap_or_default())
                }]);

                JsonRpcResponse::success(
                    id,
                    json!({
                        "content": content,
                        "isError": true
                    }),
                )
            }
        }
        Err(e) => JsonRpcResponse::success(
            id,
            json!({
                "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                "isError": true
            }),
        ),
    }
}

/// 检测异步结果并自动等待完成
///
/// 当 orchestration 工具返回 `{"type":"async", "task_id":"orch_xxx"}` 时，
/// 通过 async_task_query 轮询任务状态，直到完成或超时。
/// 这样 MCP 客户端（如 Claude Code）收到的永远是最终结果，而非中间状态。
async fn maybe_await_async_result(result: &Value, router: &SkillRouter) -> Value {
    // 只处理 {"type":"async"} 的结果
    let result_type = result.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if result_type != "async" {
        return result.clone();
    }

    let task_id = match result.get("task_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return result.clone(),
    };

    let workflow = result.get("workflow").and_then(|v| v.as_str()).unwrap_or("unknown");
    info!(
        "MCP async-await: task {} ({}) still running, polling up to {}s",
        task_id, workflow, ASYNC_POLL_TIMEOUT_SECS
    );

    let start = std::time::Instant::now();

    loop {
        // 超时检查
        if start.elapsed() > Duration::from_secs(ASYNC_POLL_TIMEOUT_SECS) {
            info!("MCP async-await: task {} timed out after {}s", task_id, ASYNC_POLL_TIMEOUT_SECS);
            return json!({
                "type": "timeout",
                "task_id": task_id,
                "workflow": workflow,
                "error": format!("任务在 {}s 内未完成，可能 AI 服务响应慢", ASYNC_POLL_TIMEOUT_SECS),
            });
        }

        // 等待后轮询
        tokio::time::sleep(Duration::from_secs(ASYNC_POLL_INTERVAL_SECS)).await;

        let query_task = format!("async_task_query: {}", task_id);
        let query_args = json!({"task_id": task_id});

        match router
            .route_with_capability(&query_task, "async_task_query", Some(query_args))
            .await
        {
            Ok(query_result) => {
                let status = query_result
                    .execution
                    .result
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match status {
                    "done" => {
                        let elapsed = start.elapsed().as_secs();
                        info!("MCP async-await: task {} completed after {}s", task_id, elapsed);
                        // 返回任务的实际结果
                        if let Some(task_result) = query_result.execution.result.get("result") {
                            return task_result.clone();
                        }
                        return query_result.execution.result;
                    }
                    "error" => {
                        info!("MCP async-await: task {} errored", task_id);
                        return query_result.execution.result;
                    }
                    _ => {
                        // 仍在运行，继续轮询
                        continue;
                    }
                }
            }
            Err(e) => {
                info!("MCP async-await: query failed for {}: {}", task_id, e);
                // 查询失败，返回原始 async 结果
                return result.clone();
            }
        }
    }
}

/// 从 inputs 列表生成 JSON Schema properties
fn build_properties_from_inputs(inputs: &[String]) -> Value {
    let mut props = serde_json::Map::new();
    for input in inputs {
        props.insert(
            input.clone(),
            json!({ "type": "string", "description": input }),
        );
    }
    Value::Object(props)
}
