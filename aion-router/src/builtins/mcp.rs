//! MCP 工具调用 Builtin
//!
//! 通过 JSON-RPC over stdin/stdout 调用外部 MCP server 的工具。
//! 支持启动子进程、初始化握手、调用工具、关闭。

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

/// MCP 工具调用
pub struct McpCall;

#[async_trait::async_trait]
impl BuiltinSkill for McpCall {
    fn name(&self) -> &'static str {
        "mcp_call"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let server_name = context.context["server"]
            .as_str()
            .ok_or_else(|| anyhow!("mcp_call requires 'server' in context"))?;

        let tool_name = context.context["tool"]
            .as_str()
            .ok_or_else(|| anyhow!("mcp_call requires 'tool' in context"))?;

        let arguments = context.context.get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        info!("mcp_call: server={}, tool={}", server_name, tool_name);

        // 查找 MCP server 配置（从环境变量或 .mcp.json）
        let server_cmd = std::env::var(format!("MCP_SERVER_{}", server_name.to_uppercase()))
            .unwrap_or_else(|_| server_name.to_string());

        // 启动 MCP server 子进程
        let mut child = Command::new(if cfg!(windows) { "cmd" } else { "sh" })
            .args(if cfg!(windows) { vec!["/c", &server_cmd] } else { vec!["-c", &server_cmd] })
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("无法启动 MCP server '{}': {}", server_name, e))?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("无法获取 stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("无法获取 stdout"))?;
        let mut reader = BufReader::new(stdout);

        // Step 1: 发送 initialize
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "aion-forge", "version": "0.1.0"}
            }
        });
        send_jsonrpc(&mut stdin, &init_req).await?;
        let _init_resp = read_jsonrpc(&mut reader).await?;

        // Step 2: 发送 tools/call
        let call_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });
        send_jsonrpc(&mut stdin, &call_req).await?;
        let call_resp = read_jsonrpc(&mut reader).await?;

        // 清理子进程
        drop(stdin);
        let _ = child.kill().await;

        // 解析结果
        if let Some(result) = call_resp.get("result") {
            Ok(json!({
                "server": server_name,
                "tool": tool_name,
                "result": result,
                "status": "ok"
            }))
        } else if let Some(error) = call_resp.get("error") {
            Ok(json!({
                "server": server_name,
                "tool": tool_name,
                "error": error,
                "status": "error"
            }))
        } else {
            Ok(json!({
                "server": server_name,
                "tool": tool_name,
                "raw_response": call_resp,
                "status": "unknown"
            }))
        }
    }
}

/// 发送 JSON-RPC 消息到 stdin
async fn send_jsonrpc(stdin: &mut tokio::process::ChildStdin, msg: &Value) -> Result<()> {
    let line = serde_json::to_string(msg)? + "\n";
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

/// 从 stdout 读取一行 JSON-RPC 响应
async fn read_jsonrpc(reader: &mut BufReader<tokio::process::ChildStdout>) -> Result<Value> {
    let mut line = String::new();
    let timeout = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        reader.read_line(&mut line),
    ).await
        .map_err(|_| anyhow!("MCP server 响应超时"))?
        .map_err(|e| anyhow!("读取 MCP 响应失败: {}", e))?;

    if timeout == 0 {
        return Err(anyhow!("MCP server 关闭了连接"));
    }

    serde_json::from_str(line.trim())
        .map_err(|e| anyhow!("MCP 响应不是有效 JSON: {}", e))
}
