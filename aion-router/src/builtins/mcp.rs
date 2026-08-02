//! MCP 工具调用 Builtin
//!
//! 通过 rmcp 3.1（官方 Rust SDK）调用外部 MCP server 的工具。
//! 支持启动子进程（stdio）并自动完成 initialize 握手、调用工具、关闭。

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

// rmcp 3.1 client API（升级依据：docs.rs/rmcp/3.1.0）：
// - `rmcp::ClientHandler`：客户端处理器（回调），空实现即用默认行为
// - `rmcp::service::serve_client(handler, transport)`：启动客户端并完成 initialize
// - `rmcp::transport::child_process::TokioChildProcess`：客户端 stdio 传输
// - `rmcp::model::CallToolRequestParams`：tools/call 参数
// - `rmcp::model::CallToolResult`：结构化结果（structured_content / content / is_error）
use rmcp::model::CallToolRequestParams;
use rmcp::service::serve_client;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::ClientHandler;

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
        .chain(std::env::current_dir().ok())
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

/// rmcp 客户端处理器（处理 server 回调，默认行为即可）
#[derive(Debug, Clone, Copy)]
struct AionMcpClientHandler;

impl ClientHandler for AionMcpClientHandler {}

/// MCP 工具调用
pub struct McpCall;

#[async_trait::async_trait]
impl BuiltinSkill for McpCall {
    fn name(&self) -> &'static str {
        "mcp_call"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let server_name = context.context["server"]
            .as_str()
            .ok_or_else(|| anyhow!("mcp_call requires 'server' in context"))?;

        let tool_name = context.context["tool"]
            .as_str()
            .ok_or_else(|| anyhow!("mcp_call requires 'tool' in context"))?;

        let arguments = context.context.get("arguments").cloned().unwrap_or_else(|| json!({}));

        // Validate server_name: only alphanumeric, underscore, hyphen allowed
        if !server_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
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

        // 构造 tokio Command（stdio：stdin/stdout piped）
        let mut cmd = if let Ok(cmd_str) = server_cmd {
            // 模式 1: 环境变量 — 使用 shell 包装（向后兼容）
            info!("mcp_call: using MCP_SERVER_{} env command", server_name.to_uppercase());
            let mut cmd = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
            cmd.arg(if cfg!(windows) { "/c" } else { "-c" }).arg(&cmd_str);
            cmd
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
            cmd
        } else {
            // 模式 3: 使用 server_name 本身作为命令（fallback）
            info!("mcp_call: using server_name '{}' directly as command", server_name);
            let mut cmd = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
            cmd.arg(if cfg!(windows) { "/c" } else { "-c" }).arg(server_name);
            cmd
        };
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);

        // 用 rmcp 客户端 stdio 传输启动子进程，并完成 initialize 握手
        let transport = TokioChildProcess::new(cmd)
            .map_err(|e| anyhow!("无法启动 MCP server '{}': {}", server_name, e))?;
        let mut service = serve_client(AionMcpClientHandler, transport)
            .await
            .map_err(|e| anyhow!("MCP server '{}' 初始化失败: {}", server_name, e))?;

        // 构造 tools/call 请求参数
        let params = match arguments {
            Value::Object(map) => CallToolRequestParams::new(tool_name.to_string()).with_arguments(map),
            other => CallToolRequestParams::new(tool_name.to_string()).with_arguments(
                serde_json::Map::from_iter([("value".to_string(), other)]),
            ),
        };

        // 调用工具
        let result = match service.call_tool(params).await {
            Ok(result) => result,
            Err(e) => {
                // 传输/协议错误：返回 status=error（与旧实现一致，不向上抛异常）
                return Ok(json!({
                    "server": server_name,
                    "tool": tool_name,
                    "error": {"message": e.to_string()},
                    "status": "error"
                }));
            }
        };

        // 清理连接（优雅关闭，子进程会被终止）
        let _ = service.close().await;

        // 解析结果
        if result.is_error == Some(true) {
            Ok(json!({
                "server": server_name,
                "tool": tool_name,
                "error": result.structured_content.clone().unwrap_or_else(|| json!(result.content)),
                "status": "error"
            }))
        } else if let Some(structured) = result.structured_content {
            Ok(json!({
                "server": server_name,
                "tool": tool_name,
                "result": structured,
                "status": "ok"
            }))
        } else {
            Ok(json!({
                "server": server_name,
                "tool": tool_name,
                "result": serde_json::to_value(&result).unwrap_or_else(|_| json!(result.content)),
                "status": "ok"
            }))
        }
    }
}
