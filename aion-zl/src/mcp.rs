//! MCP server protocol handler

use crate::engine::Engine;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write as IoWrite};

#[derive(Debug, Serialize, Deserialize)]
pub struct McpReq {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpResp {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

impl McpResp {
    fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    fn err(id: serde_json::Value, code: i32, msg: String) -> Self {
        Self {
            jsonrpc: "2.0".into(), id, result: None,
            error: Some(serde_json::json!({ "code": code, "message": msg })),
        }
    }
}

pub async fn run(engine: &Engine) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }

        let req: McpReq = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let r = McpResp::err(serde_json::Value::Null, -32700, format!("Parse error: {}", e));
                writeln!(stdout, "{}", serde_json::to_string(&r)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let resp = handle(engine, &req).await;
        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}

async fn handle(engine: &Engine, req: &McpReq) -> McpResp {
    match req.method.as_str() {
        "initialize" => McpResp::ok(req.id.clone(), serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "aion-zl", "version": "0.2.0" }
        })),
        "notifications/initialized" => McpResp::ok(req.id.clone(), serde_json::json!({})),
        "tools/list" => McpResp::ok(req.id.clone(), serde_json::json!({ "tools": tools() })),
        "tools/call" => {
            let name = req.params["name"].as_str().unwrap_or("");
            let args = &req.params["arguments"];
            match call(engine, name, args).await {
                Ok(v) => {
                    let text = serde_json::to_string_pretty(&v).unwrap_or_default();
                    McpResp::ok(req.id.clone(), serde_json::json!({
                        "content": [{ "type": "text", "text": text }],
                        "isError": false
                    }))
                }
                Err(e) => McpResp::ok(req.id.clone(), serde_json::json!({
                    "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                    "isError": true
                })),
            }
        }
        _ => McpResp::err(req.id.clone(), -32601, format!("Unknown method: {}", req.method)),
    }
}

async fn call(engine: &Engine, tool: &str, args: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let task = args["task"].as_str().unwrap_or("");
    match tool {
        "task_dialectic" => Ok(serde_json::to_value(engine.task_dialectic(task).await?)?),
        "contradiction_analyze" => Ok(serde_json::to_value(engine.contradiction_analyze(task).await?)?),
        "strategic_plan" => Ok(serde_json::to_value(engine.strategic_plan(task).await?)?),
        "dialectical_retry" => {
            let max = args["max_attempts"].as_u64().unwrap_or(3) as u32;
            Ok(serde_json::to_value(engine.dialectical_retry(task, max).await?)?)
        }
        "compile_contract" => {
            Ok(serde_json::to_value(engine.compile_contract(task).await?)?)
        }
        "check_sufficiency" => {
            let contract: crate::contract::TaskContract = serde_json::from_value(args["contract"].clone())
                .map_err(|e| anyhow::anyhow!("Invalid contract: {}", e))?;
            let context = args["context"].as_str().unwrap_or("");
            Ok(serde_json::to_value(engine.check_sufficiency(&contract, context).await?)?)
        }
        "verify_result" => {
            let contract: crate::contract::TaskContract = serde_json::from_value(args["contract"].clone())
                .map_err(|e| anyhow::anyhow!("Invalid contract: {}", e))?;
            let result = args["result"].as_str().unwrap_or("");
            Ok(serde_json::to_value(engine.verify_result(&contract, result).await?)?)
        }
        "detect_drift" => {
            let contract: crate::contract::TaskContract = serde_json::from_value(args["contract"].clone())
                .map_err(|e| anyhow::anyhow!("Invalid contract: {}", e))?;
            let state = args["current_state"].as_str().unwrap_or("");
            Ok(serde_json::to_value(engine.detect_drift(&contract, state).await?)?)
        }
        _ => anyhow::bail!("Unknown tool: {}", tool),
    }
}

fn tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "task_dialectic",
            "description": "Execute thesis-antithesis-synthesis dialectical analysis. Three AI passes: propose, critique, synthesize.",
            "inputSchema": {
                "type": "object",
                "properties": { "task": { "type": "string", "description": "Task to analyze dialectically" } },
                "required": ["task"]
            }
        }),
        serde_json::json!({
            "name": "contradiction_analyze",
            "description": "Identify principal/secondary contradictions. Find bottlenecks, recommend resource allocation.",
            "inputSchema": {
                "type": "object",
                "properties": { "task": { "type": "string", "description": "Complex task to analyze" } },
                "required": ["task"]
            }
        }),
        serde_json::json!({
            "name": "strategic_plan",
            "description": "Three-phase strategic plan (defense/stalemate/offense) mapping to aion-forge capabilities.",
            "inputSchema": {
                "type": "object",
                "properties": { "task": { "type": "string", "description": "Complex task to plan" } },
                "required": ["task"]
            }
        }),
        serde_json::json!({
            "name": "dialectical_retry",
            "description": "Execute task with learning-based retry. On failure: analyze root cause, learn, adapt, retry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "Task to execute" },
                    "max_attempts": { "type": "integer", "description": "Max retries (default 3)", "default": 3 }
                },
                "required": ["task"]
            }
        }),
        serde_json::json!({
            "name": "compile_contract",
            "description": "Compile a natural language task into a structured contract defining acceptance criteria, expected outputs, and verification method. Use before execution to establish the 'north star'.",
            "inputSchema": {
                "type": "object",
                "properties": { "task": { "type": "string", "description": "Task to compile into a contract" } },
                "required": ["task"]
            }
        }),
        serde_json::json!({
            "name": "check_sufficiency",
            "description": "P0 Sensor: Check if context is sufficient before execution. Prevents premature action when understanding is incomplete.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract": { "type": "object", "description": "TaskContract from compile_contract" },
                    "context": { "type": "string", "description": "Available context/information" }
                },
                "required": ["contract", "context"]
            }
        }),
        serde_json::json!({
            "name": "verify_result",
            "description": "P1 Sensor: Verify execution result against task contract. Checks each acceptance criterion and returns pass/fail with evidence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract": { "type": "object", "description": "TaskContract from compile_contract" },
                    "result": { "type": "string", "description": "Execution result to verify" }
                },
                "required": ["contract", "result"]
            }
        }),
        serde_json::json!({
            "name": "detect_drift",
            "description": "P1 Sensor: Detect if execution is drifting away from original goal. Returns correction suggestions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract": { "type": "object", "description": "TaskContract from compile_contract" },
                    "current_state": { "type": "string", "description": "Current execution state description" }
                },
                "required": ["contract", "current_state"]
            }
        }),
    ]
}
