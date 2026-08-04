//! MCP Client — 连接外部 MCP 服务器，发现并调用其工具
//!
//! 支持两种传输方式：
//! - **Stdio**：启动子进程，通过 stdin/stdout 交换 JSON-RPC
//! - **Streamable HTTP**：HTTP 传输（旧 SSE 模式的现代等价物）
//!
//! 连接后自动发现工具并注册到 CapabilityRegistry。
//!
//! # 实现说明（升级 2026-08-02）
//! 底层由手写 JSON-RPC-over-stdio 替换为 **rmcp 3.1**（官方 Rust SDK，docs.rs/rmcp/3.1.0）：
//! - `rmcp::client::ClientHandler`（handler trait）+ `rmcp::service::serve_client`
//! - 客户端 stdio 传输：`rmcp::transport::child_process::TokioChildProcess`
//! - streamable HTTP 传输（原 SSE 预留位）：`rmcp::transport::StreamableHttpClientTransport::from_uri`
//!   （feature `transport-streamable-http-client-reqwest`）
//! 公开结构与方法签名保持不变（`McpClientManager::new / load_from_config / connect /
//! connect_stdio / call_tool / all_tools / connected_servers / disconnect / shutdown`）。
//! initialize 握手、协议版本协商、tools/list 分页等由 rmcp 自动处理。

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;

use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::service::{serve_client, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ClientHandler, RoleClient};

/// MCP 传输方式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpTransport {
    /// stdio 模式
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Streamable HTTP 模式（旧 SSE 的现代等价物，rmcp 支持）
    Sse { url: String },
}

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// 传输配置
    #[serde(flatten)]
    pub transport: McpTransport,
    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// 从远程 MCP 服务器发现的工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// 工具名称
    pub name: String,
    /// 工具描述
    #[serde(default)]
    pub description: String,
    /// 输入 JSON Schema
    #[serde(default)]
    pub input_schema: Value,
    /// 所属服务器名称
    #[serde(skip)]
    pub server_name: String,
}

/// rmcp 客户端 handler（处理 MCP server 发来的回调请求）。
///
/// 使用默认行为（ping 应答、通知忽略等）；`get_info` 返回默认 ClientInfo。
/// 因 `ClientHandler` 的 `get_info` 有默认实现，空实现即可。
#[derive(Debug, Clone, Copy, Default)]
struct AionMcpClientHandler;

impl ClientHandler for AionMcpClientHandler {}

/// 活跃的 MCP 服务器连接
struct McpServerHandle {
    /// rmcp 运行中的客户端服务（内部持有传输与后台任务）
    service: RunningService<RoleClient, AionMcpClientHandler>,
    /// 发现的工具
    tools: Vec<McpTool>,
}

/// MCP 配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfigFile {
    /// 服务器配置（key = 服务器名称）
    pub servers: HashMap<String, McpServerConfig>,
}

/// MCP Client 管理器
pub struct McpClientManager {
    servers: HashMap<String, McpServerHandle>,
}

impl Default for McpClientManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClientManager {
    /// 创建空管理器
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// 从配置文件加载并连接所有服务器
    pub async fn load_from_config(config_path: &Path) -> Result<Self> {
        let mut manager = Self::new();

        if !config_path.exists() {
            tracing::info!("no MCP config found at {}, skipping", config_path.display());
            return Ok(manager);
        }

        let content = std::fs::read_to_string(config_path)?;
        let config: McpConfigFile = serde_json::from_str(&content)?;

        for (name, server_config) in &config.servers {
            match manager.connect(name, server_config).await {
                Ok(tool_count) => {
                    tracing::info!(
                        server = %name,
                        tools = tool_count,
                        "MCP server connected"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        server = %name,
                        error = %e,
                        "failed to connect MCP server"
                    );
                }
            }
        }

        Ok(manager)
    }

    /// 连接一个 MCP 服务器
    pub async fn connect(&mut self, name: &str, config: &McpServerConfig) -> Result<usize> {
        match &config.transport {
            McpTransport::Stdio { command, args } => self.connect_stdio(name, command, args, &config.env).await,
            // 升级：rmcp 3.1 支持 streamable HTTP client（SSE 传输的现代规范替代），
            // 不再返回 "not yet implemented"。
            McpTransport::Sse { url } => self.connect_streamable_http(name, url).await,
        }
    }

    /// Stdio 模式连接（rmcp `TokioChildProcess` + `serve_client`）
    async fn connect_stdio(
        &mut self,
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<usize> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        // 设置环境变量（支持 ${VAR} 引用系统变量）
        for (k, v) in env {
            let resolved = Self::resolve_env_var(v);
            cmd.env(k, resolved);
        }

        let transport = TokioChildProcess::new(cmd)
            .map_err(|e| anyhow!("failed to start MCP server '{}' ({}): {}", name, command, e))?;

        // serve_client 自动完成 initialize 握手与协议版本协商
        let service = serve_client(AionMcpClientHandler, transport)
            .await
            .map_err(|e| anyhow!("failed to initialize MCP server '{}' ({}): {}", name, command, e))?;

        // 记录协商出的协议版本（rmcp 内部已完成握手）
        if let Some(info) = service.peer().peer_info() {
            tracing::info!(
                server = %name,
                protocol = %info.protocol_version,
                "MCP initialized"
            );
        }

        // 发现工具（tools/list，rmcp 自动处理分页）
        let list = service
            .peer()
            .list_tools(None)
            .await
            .map_err(|e| anyhow!("failed to list tools on MCP server '{}': {}", name, e))?;
        let tools: Vec<McpTool> = list
            .tools
            .into_iter()
            .map(|tool| mcp_tool_from_rmcp(tool, name))
            .collect();
        let tool_count = tools.len();

        self.servers
            .insert(name.to_string(), McpServerHandle { service, tools });
        Ok(tool_count)
    }

    /// Streamable HTTP 模式连接（rmcp `StreamableHttpClientTransport`，替代旧 SSE）
    async fn connect_streamable_http(&mut self, name: &str, url: &str) -> Result<usize> {
        let transport = StreamableHttpClientTransport::from_uri(url.to_string());
        let service = serve_client(AionMcpClientHandler, transport)
            .await
            .map_err(|e| anyhow!("failed to initialize MCP server '{}' ({}): {}", name, url, e))?;

        if let Some(info) = service.peer().peer_info() {
            tracing::info!(
                server = %name,
                protocol = %info.protocol_version,
                "MCP streamable-http initialized"
            );
        }

        let list = service
            .peer()
            .list_tools(None)
            .await
            .map_err(|e| anyhow!("failed to list tools on MCP server '{}': {}", name, e))?;
        let tools: Vec<McpTool> = list
            .tools
            .into_iter()
            .map(|tool| mcp_tool_from_rmcp(tool, name))
            .collect();
        let tool_count = tools.len();

        self.servers
            .insert(name.to_string(), McpServerHandle { service, tools });
        Ok(tool_count)
    }

    /// 调用远程 MCP 工具
    pub async fn call_tool(&mut self, server_name: &str, tool_name: &str, arguments: Value) -> Result<Value> {
        let handle = self
            .servers
            .get_mut(server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", server_name))?;

        // 验证工具存在
        if !handle.tools.iter().any(|t| t.name == tool_name) {
            return Err(anyhow!(
                "tool '{}' not found on MCP server '{}'",
                tool_name,
                server_name
            ));
        }

        let args = match arguments {
            Value::Object(map) => map,
            _ => return Err(anyhow!("tool arguments for '{}' must be a JSON object", tool_name)),
        };
        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(args);

        let result = handle
            .service
            .call_tool(params)
            .await
            .map_err(|e| anyhow!("MCP tool call '{}' on '{}' failed: {}", tool_name, server_name, e))?;

        // rmcp 3.1 的 CallToolResult（SEP-2322）：is_error 标记工具级错误
        if result.is_error == Some(true) {
            let detail = result
                .structured_content
                .clone()
                .unwrap_or_else(|| json!({ "content": result.content }));
            return Err(anyhow!(
                "MCP tool '{}' on '{}' returned error: {}",
                tool_name,
                server_name,
                detail
            ));
        }

        // 优先返回 structuredContent（新协议的结构化结果），否则返回完整协议结果
        if let Some(structured) = result.structured_content {
            Ok(structured)
        } else {
            serde_json::to_value(result).map_err(|e| anyhow!("failed to serialize tool result: {}", e))
        }
    }

    /// 获取所有已发现的工具
    pub fn all_tools(&self) -> Vec<McpTool> {
        let mut tools = Vec::new();
        for (name, handle) in &self.servers {
            for tool in &handle.tools {
                let mut t = tool.clone();
                t.server_name = name.clone();
                tools.push(t);
            }
        }
        tools
    }

    /// 获取已连接的服务器列表
    pub fn connected_servers(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    /// 断开指定服务器（rmcp `RunningService::close` 会优雅关闭传输并终止子进程）
    pub async fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(mut handle) = self.servers.remove(name) {
            // close() 关闭传输（stdio 下会先等子进程正常退出，超时则 kill）
            if let Err(e) = handle.service.close().await {
                tracing::warn!(server = %name, error = %e, "MCP server close failed");
            }
            tracing::info!(server = %name, "MCP server disconnected");
        }
        Ok(())
    }

    /// 关闭所有连接
    pub async fn shutdown(&mut self) {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        for name in names {
            let _ = self.disconnect(&name).await;
        }
    }

    /// 解析环境变量引用 ${VAR}
    fn resolve_env_var(value: &str) -> String {
        if value.starts_with("${") && value.ends_with('}') {
            let var_name = &value[2..value.len() - 1];
            std::env::var(var_name).unwrap_or_default()
        } else {
            value.to_string()
        }
    }
}

/// 将 rmcp 3.1 的 `Tool` 转换为本 crate 的 `McpTool`。
///
/// 依据 docs.rs/rmcp/3.1.0/rmcp/model/struct.Tool.html：
/// `Tool { name: Cow<'static,str>, description: Option<Cow>, input_schema: Arc<JsonObject>, .. }`
fn mcp_tool_from_rmcp(tool: Tool, server_name: &str) -> McpTool {
    McpTool {
        name: tool.name.to_string(),
        description: tool.description.as_deref().unwrap_or("").to_string(),
        input_schema: tool.schema_as_json_value(),
        server_name: server_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_file_parse() {
        let json = r#"{
            "servers": {
                "echo": {
                    "transport": "stdio",
                    "command": "echo",
                    "args": ["hello"],
                    "env": {}
                }
            }
        }"#;
        let config: McpConfigFile = serde_json::from_str(json).unwrap();
        assert!(config.servers.contains_key("echo"));
    }

    #[test]
    fn test_mcp_tool_serde() {
        let tool = McpTool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            server_name: "filesystem".to_string(),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: McpTool = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "read_file");
    }

    #[test]
    fn test_resolve_env_var() {
        std::env::set_var("TEST_MCP_VAR", "resolved_value");
        assert_eq!(McpClientManager::resolve_env_var("${TEST_MCP_VAR}"), "resolved_value");
        assert_eq!(McpClientManager::resolve_env_var("literal"), "literal");
        std::env::remove_var("TEST_MCP_VAR");
    }

    #[test]
    fn test_mcp_client_manager_new() {
        let manager = McpClientManager::new();
        assert!(manager.all_tools().is_empty());
        assert!(manager.connected_servers().is_empty());
    }
}
