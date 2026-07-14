//! MCP 工具调用 Builtin
//!
//! 通过 JSON-RPC over stdin/stdout 调用外部 MCP server 的工具。
//! 支持启动子进程、初始化握手、调用工具、关闭。

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

/// MCP 配置文件结构（.mcp.json 格式）
#[derive(Debug, Deserialize)]
struct McpConfigFile {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpServerConfigEntry>,
}

/// .mcp.json 中单个 MCP server 的配置项
#[derive(Debug, Clone, Deserialize)]
struct McpServerConfigEntry {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// 尝试从 .mcp.json 加载指定 MCP server 的配置
///
/// 查找顺序：
/// 1. `AION_WORKSPACE_ROOT` 环境变量指向的目录
/// 2. 当前工作目录
/// 3. 逐级向上查找父目录（直到找到 `.mcp.json` 或到达根目录）
fn load_mcp_server_config(server_name: &str) -> Option<McpServerConfigEntry> {
    let candidates = std::env::var("AION_WORKSPACE_ROOT")
        .ok()
        .map(PathBuf::from)
        .into_iter()
        .chain(std::env::current_dir().ok().into_iter())
        .flat_map(|mut dir| {
            // 收集从当前目录往上直到根目录的所有路径
            let mut dirs = Vec::new();
            loop {
                dirs.push(dir.clone());
                if !dir.pop() {
                    break;
                }
            }
            dirs
        });

    for root in candidates {
        let mcp_path = root.join(".mcp.json");
        if mcp_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&mcp_path) {
                if let Ok(config) = serde_json::from_str::<McpConfigFile>(&content) {
                    if let Some(entry) = config.mcp_servers.get(server_name) {
                        return Some(entry.clone());
                    }
                }
            }
        }
    }
    None
}

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

        // Validate server_name: only alphanumeric, underscore, hyphen allowed
        if !server_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(anyhow!(
                "Invalid MCP server name '{}': only [a-zA-Z0-9_-] allowed",
                server_name
            ));
        }

        info!("mcp_call: server={}, tool={}", server_name, tool_name);

        // 查找 MCP server 配置：
        // 1. 优先从环境变量 MCP_SERVER_{NAME} 读取
        // 2. 其次从 .mcp.json 读取
        // 3. 最后直接用 server_name 作为命令
        let server_cmd = std::env::var(format!("MCP_SERVER_{}", server_name.to_uppercase()));

        let (mut child, extra_env) = if let Ok(cmd) = server_cmd {
            // 模式 1: 环境变量 — 使用 shell 包装（向后兼容）
            let child = Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                .args(if cfg!(windows) { vec!["/c", &cmd] } else { vec!["-c", &cmd] })
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| anyhow!("无法启动 MCP server '{}' (env): {}", server_name, e))?;
            (child, HashMap::new())
        } else if let Some(config) = load_mcp_server_config(server_name) {
            // 模式 2: .mcp.json 配置 — 直接执行命令并设置环境变量
            info!("mcp_call: found '{}' in .mcp.json", server_name);
            let mut cmd = Command::new(&config.command);
            if !config.args.is_empty() {
                cmd.args(&config.args);
            }
            // 设置环境变量
            for (k, v) in &config.env {
                cmd.env(k, v);
            }
            let child = cmd
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| anyhow!(
                    "无法启动 MCP server '{}' (command='{}'): {}",
                    server_name, config.command, e
                ))?;
            (child, config.env)
        } else {
            // 模式 3: 使用 server_name 本身作为命令（fallback）
            let child = Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                .args(if cfg!(windows) { vec!["/c", server_name] } else { vec!["-c", server_name] })
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| anyhow!("无法启动 MCP server '{}' (fallback): {}", server_name, e))?;
            (child, HashMap::new())
        };

        // 记录环境变量（不含敏感值）
        if !extra_env.is_empty() {
            let keys: Vec<&String> = extra_env.keys().collect();
            info!("mcp_call: server '{}' env keys: {:?}", server_name, keys);
        }

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
