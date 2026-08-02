//! MCP stdio server for Aion Forge, built on the official rmcp 3.1 SDK.
//!
//! `run()` replaces the previous hand-written JSON-RPC stdio loop with rmcp's
//! `ServerHandler` trait + `stdio()` transport (https://docs.rs/rmcp/3.1.0/rmcp/).
//! The public helpers `initialize_response` / `tools_list_response` /
//! `write_json_line` are kept with unchanged signatures so the crate-level
//! tests (`tests/mcp_contract.rs`) keep passing.

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde_json::{json, Value};

use aion_router::agent_runtime::AgentRuntime;
use aion_router::coordinator::MultiAgentCoordinator;
use aion_router::SkillRouter;
use aion_types::agent_message::{AgentRef, AgentRole};
use aion_types::types::RouterPaths;

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
        JsonObject, ListToolsResult, MetaObject, PaginatedRequestParams, ProtocolVersion,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    transport::stdio,
};

const ASYNC_POLL_TIMEOUT_SECS: u64 = 300;
const ASYNC_POLL_INTERVAL_SECS: u64 = 5;

/// The negotiated MCP protocol version advertised by `initialize`.
///
/// rmcp 3.1 targets the `2026-07-28` specification while remaining compatible
/// with `2025-11-25`; `ProtocolVersion::LATEST` defaults to `2025-11-25`
/// (https://docs.rs/rmcp/3.1.0/rmcp/model/struct.ProtocolVersion.html).
const PROTOCOL_VERSION: &str = "2025-11-25";

/// Build the MCP initialize response for Aion Forge.
pub fn initialize_response(id: Value) -> Value {
    success(
        id,
        json!({
            "protocolVersion": PROTOCOL_VERSION,
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
    let tools: Vec<Value> = crate::catalog::entries()
        .into_iter()
        .map(|entry| {
            json!({
                "name": entry.name,
                "description": entry.description,
                "inputSchema": entry.input_schema,
                "requiresApproval": entry.requires_approval,
            })
        })
        .collect();

    success(id, json!({"tools": tools}))
}

/// Write exactly one compact JSON-RPC line to the protocol output stream.
pub fn write_json_line(output: &mut impl Write, response: &Value) -> Result<()> {
    serde_json::to_writer(&mut *output, response)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

/// Run the MCP stdio server on rmcp's official stdio transport.
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

    let server = ForgeMcpServer {
        router: Arc::new(router),
    };
    // `ServiceExt::serve` finishes the MCP `initialize` handshake, then
    // `RunningService::waiting` runs the transport loop until the client
    // closes stdin (https://docs.rs/rmcp/3.1.0/rmcp/service/trait.ServiceExt.html).
    let running = server.serve(stdio()).await?;
    running.waiting().await?;
    Ok(())
}

/// The rmcp [`ServerHandler`] exposing the Aion Forge tool catalog.
#[derive(Clone)]
struct ForgeMcpServer {
    router: Arc<SkillRouter>,
}

impl ServerHandler for ForgeMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("aion-forge", env!("CARGO_PKG_VERSION")))
            .with_protocol_version(ProtocolVersion::LATEST)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = crate::catalog::entries()
            .into_iter()
            .map(|entry| {
                // Preserve the legacy `requiresApproval` flag as tool `_meta`.
                let mut meta = MetaObject::new();
                meta.insert(
                    "requiresApproval".to_string(),
                    Value::Bool(entry.requires_approval),
                );
                Tool::new(entry.name, entry.description, schema_to_object(entry.input_schema))
                    .with_meta(meta)
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let tool_name = request.name.to_string();
        if tool_name.is_empty() {
            return Err(McpError::invalid_params(
                "Missing 'name' in tools/call params",
                None,
            ));
        }
        let arguments = request.arguments.map(Value::Object).unwrap_or_else(|| json!({}));
        Ok(self.run_tool(&tool_name, arguments).await.into())
    }
}

impl ForgeMcpServer {
    /// Route one `tools/call` through the Aion Forge skill router, reusing the
    /// original passthrough / routing / async-poll logic.
    async fn run_tool(&self, tool_name: &str, arguments: Value) -> CallToolResult {
        if passthrough_enabled() {
            if let Some(instruction) = aion_intel::synth::ai_instruction_for(tool_name) {
                let text = arguments
                    .get("text")
                    .or_else(|| arguments.get("input"))
                    .or_else(|| arguments.get("query"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                return CallToolResult::success(vec![ContentBlock::text(format!(
                    "[Instruction]: {instruction}\n\n[Input]:\n{text}"
                ))]);
            }
        }

        let task = arguments
            .get("text")
            .or_else(|| arguments.get("query"))
            .and_then(Value::as_str)
            .map(|text| format!("{tool_name}: {text}"))
            .unwrap_or_else(|| format!("{tool_name}: {arguments}"));

        match self
            .router
            .route_with_capability(&task, tool_name, Some(arguments))
            .await
        {
            Ok(result) if result.execution.status == "ok" => {
                let final_result = await_async_result(&result.execution.result, &self.router).await;
                let text = serde_json::to_string_pretty(&final_result)
                    .unwrap_or_else(|_| final_result.to_string());
                CallToolResult::success(vec![ContentBlock::text(text)])
            }
            Ok(result) => CallToolResult::error(vec![ContentBlock::text(format!(
                "Error: {}",
                result.execution.error.unwrap_or_default()
            ))]),
            Err(error) => {
                CallToolResult::error(vec![ContentBlock::text(format!("Error: {error}"))])
            }
        }
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

/// Convert a raw JSON-schema [`Value`] into the rmcp [`JsonObject`]
/// (serde_json map) expected by [`Tool::new`].
fn schema_to_object(schema: Value) -> JsonObject {
    match schema {
        Value::Object(map) => map,
        _ => JsonObject::new(),
    }
}

fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}
