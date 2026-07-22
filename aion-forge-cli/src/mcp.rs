use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use aion_router::agent_runtime::AgentRuntime;
use aion_router::coordinator::MultiAgentCoordinator;
use aion_router::SkillRouter;
use aion_types::agent_message::{AgentRef, AgentRole};
use aion_types::types::RouterPaths;

const ASYNC_POLL_TIMEOUT_SECS: u64 = 300;
const ASYNC_POLL_INTERVAL_SECS: u64 = 5;

/// Build the MCP initialize response for Aion Forge.
pub fn initialize_response(id: Value) -> Value {
    success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {
                "name": "aion-forge",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

/// Build the current MCP tool catalog response.
pub fn tools_list_response(id: Value) -> Value {
    let registry = aion_types::capability_registry::CapabilityRegistry::builtin();
    let mut tools: Vec<Value> = registry
        .definitions()
        .map(|capability| {
            let input_schema = if capability.parameters_schema.is_null() || capability.parameters_schema == json!({}) {
                json!({
                    "type": "object",
                    "properties": properties_from_inputs(&capability.inputs),
                    "required": capability.inputs,
                })
            } else {
                capability.parameters_schema.clone()
            };
            json!({
                "name": capability.name,
                "description": capability.description,
                "inputSchema": input_schema,
                "requiresApproval": capability.requires_approval,
            })
        })
        .collect();

    tools.push(json!({
        "name": "async_task_query",
        "description": "Query an asynchronous orchestration task by task_id.",
        "inputSchema": {
            "type": "object",
            "properties": {"task_id": {"type": "string"}}
        }
    }));

    success(id, json!({"tools": tools}))
}

/// Write exactly one compact JSON-RPC line to the protocol output stream.
pub fn write_json_line(output: &mut impl Write, response: &Value) -> Result<()> {
    serde_json::to_writer(&mut *output, response)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

/// Run the MCP stdio server.
pub async fn run(paths: RouterPaths) -> Result<()> {
    aion_router::learner::init_learner(&paths.workspace_root);
    let global_bus = aion_router::message_bus::init_global_bus(128);
    let bus = Arc::new((*global_bus).clone());
    let router = SkillRouter::new(paths)?;
    let mut coordinator = MultiAgentCoordinator::new(Arc::clone(&bus));
    for (id, role) in [
        ("orchestrator-0", AgentRole::Orchestrator),
        ("executor-0", AgentRole::Executor),
        ("executor-1", AgentRole::Executor),
    ] {
        match AgentRuntime::new(id, role.clone(), Vec::new(), router.paths().clone(), Arc::clone(&bus)) {
            Ok(runtime) => {
                runtime.spawn();
                coordinator.register_agent(AgentRef::local(id, role));
                tracing::info!(agent = id, "MCP agent runtime started");
            }
            Err(error) => tracing::warn!(agent = id, %error, "MCP agent runtime failed to start"),
        }
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(stdin.lock(), stdout.lock(), &router).await
}

async fn serve(reader: impl BufRead, mut output: impl Write, router: &SkillRouter) -> Result<()> {
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_json_line(
                    &mut output,
                    &failure(Value::Null, -32700, &format!("Parse error: {error}")),
                )?;
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        if id.is_none() || method.starts_with("notifications/") || method == "initialized" {
            continue;
        }
        let id = id.unwrap_or(Value::Null);

        let response = if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            failure(id, -32600, "Invalid JSON-RPC version")
        } else {
            match method {
                "initialize" => initialize_response(id),
                "tools/list" => tools_list_response(id),
                "tools/call" => {
                    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
                    call_tool(id, &params, router).await
                }
                _ => failure(id, -32601, &format!("Method not found: {method}")),
            }
        };
        write_json_line(&mut output, &response)?;
    }
    Ok(())
}

async fn call_tool(id: Value, params: &Value, router: &SkillRouter) -> Value {
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    if tool_name.is_empty() {
        return failure(id, -32602, "Missing 'name' in tools/call params");
    }

    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    if passthrough_enabled() {
        if let Some(instruction) = aion_intel::synth::ai_instruction_for(tool_name) {
            let text = arguments
                .get("text")
                .or_else(|| arguments.get("input"))
                .or_else(|| arguments.get("query"))
                .and_then(Value::as_str)
                .unwrap_or("");
            return success(
                id,
                json!({
                    "content": [{"type": "text", "text": format!("[Instruction]: {instruction}\n\n[Input]:\n{text}")}],
                    "isError": false
                }),
            );
        }
    }

    let task = arguments
        .get("text")
        .or_else(|| arguments.get("query"))
        .and_then(Value::as_str)
        .map(|text| format!("{tool_name}: {text}"))
        .unwrap_or_else(|| format!("{tool_name}: {arguments}"));

    match router.route_with_capability(&task, tool_name, Some(arguments)).await {
        Ok(result) if result.execution.status == "ok" => {
            let final_result = await_async_result(&result.execution.result, router).await;
            let text = serde_json::to_string_pretty(&final_result).unwrap_or_else(|_| final_result.to_string());
            success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }],
                    "isError": false
                }),
            )
        }
        Ok(result) => success(
            id,
            json!({
                "content": [{"type": "text", "text": format!("Error: {}", result.execution.error.unwrap_or_default())}],
                "isError": true
            }),
        ),
        Err(error) => success(
            id,
            json!({
                "content": [{"type": "text", "text": format!("Error: {error}")}],
                "isError": true
            }),
        ),
    }
}

async fn await_async_result(result: &Value, router: &SkillRouter) -> Value {
    if result.get("type").and_then(Value::as_str) != Some("async") {
        return result.clone();
    }
    let Some(task_id) = result.get("task_id").and_then(Value::as_str) else {
        return result.clone();
    };
    let task_id = task_id.to_string();
    let workflow = result
        .get("workflow")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let started = std::time::Instant::now();

    loop {
        if started.elapsed() > Duration::from_secs(ASYNC_POLL_TIMEOUT_SECS) {
            return json!({
                "type": "timeout",
                "task_id": task_id,
                "workflow": workflow,
                "error": format!("task did not finish within {ASYNC_POLL_TIMEOUT_SECS}s")
            });
        }

        tokio::time::sleep(Duration::from_secs(ASYNC_POLL_INTERVAL_SECS)).await;
        let arguments = json!({"task_id": task_id});
        match router
            .route_with_capability(
                &format!("async_task_query: {task_id}"),
                "async_task_query",
                Some(arguments),
            )
            .await
        {
            Ok(query) => match query.execution.result.get("status").and_then(Value::as_str) {
                Some("done") => {
                    return query
                        .execution
                        .result
                        .get("result")
                        .cloned()
                        .unwrap_or(query.execution.result)
                }
                Some("error") => return query.execution.result,
                _ => continue,
            },
            Err(error) => {
                tracing::warn!(%error, task_id, "MCP async task query failed");
                return result.clone();
            }
        }
    }
}

fn passthrough_enabled() -> bool {
    std::env::var("AI_PASSTHROUGH")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(false)
}

fn properties_from_inputs(inputs: &[String]) -> Value {
    let properties = inputs
        .iter()
        .map(|input| (input.clone(), json!({"type": "string", "description": input})))
        .collect();
    Value::Object(properties)
}

fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn failure(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message}
    })
}
