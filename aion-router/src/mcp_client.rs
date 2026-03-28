//! MCP Client — 连接外部 MCP 服务器，发现并调用其工具
//!
//! 支持两种传输方式：
//! - **Stdio**：启动子进程，通过 stdin/stdout 交换 JSON-RPC
//! - **Sse**：HTTP SSE 连接（预留，MVP 先实现 Stdio）
//!
//! 连接后自动发现工具并注册到 CapabilityRegistry。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

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
    /// SSE 模式（预留）
    Sse {
        url: String,
    },
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

/// 活跃的 MCP 服务器连接
struct McpServerHandle {
    /// 子进程（Stdio 模式）
    child: Child,
    /// stdin writer
    stdin: tokio::process::ChildStdin,
    /// stdout reader
    stdout: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    /// 发现的工具
    tools: Vec<McpTool>,
    /// 下一个请求 ID
    next_id: u64,
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
            McpTransport::Stdio { command, args } => {
                self.connect_stdio(name, command, args, &config.env).await
            }
            McpTransport::Sse { url } => {
                Err(anyhow!("SSE transport not yet implemented for {}", url))
            }
        }
    }

    /// Stdio 模式连接
    async fn connect_stdio(
        &mut self,
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<usize> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);

        // 设置环境变量（支持 ${VAR} 引用系统变量）
        for (k, v) in env {
            let resolved = Self::resolve_env_var(v);
            cmd.env(k, resolved);
        }

        let mut child = cmd.spawn().map_err(|e| {
            anyhow!("failed to start MCP server '{}' ({}): {}", name, command, e)
        })?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin for {}", name))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout for {}", name))?;
        let stdout = Arc::new(Mutex::new(BufReader::new(stdout)));

        let mut handle = McpServerHandle {
            child,
            stdin,
            stdout,
            tools: Vec::new(),
            next_id: 1,
        };

        // 初始化握手
        Self::initialize(&mut handle, name).await?;

        // 发现工具
        let tools = Self::list_tools(&mut handle, name).await?;
        let tool_count = tools.len();
        handle.tools = tools;

        self.servers.insert(name.to_string(), handle);
        Ok(tool_count)
    }

    /// 发送 initialize 请求
    async fn initialize(handle: &mut McpServerHandle, name: &str) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": handle.next_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "aion-forge",
                    "version": "0.5.0"
                }
            }
        });
        handle.next_id += 1;

        let response = Self::send_request(handle, &request).await?;

        tracing::info!(
            server = %name,
            protocol = %response["result"]["protocolVersion"].as_str().unwrap_or("unknown"),
            "MCP initialized"
        );

        Ok(())
    }

    /// 发送 tools/list 请求
    async fn list_tools(handle: &mut McpServerHandle, name: &str) -> Result<Vec<McpTool>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": handle.next_id,
            "method": "tools/list",
            "params": {}
        });
        handle.next_id += 1;

        let response = Self::send_request(handle, &request).await?;

        let tools_array = response["result"]["tools"]
            .as_array()
            .ok_or_else(|| anyhow!("tools/list returned no tools array for {}", name))?;

        let mut tools = Vec::new();
        for tool_val in tools_array {
            let tool = McpTool {
                name: tool_val["name"].as_str().unwrap_or("").to_string(),
                description: tool_val["description"].as_str().unwrap_or("").to_string(),
                input_schema: tool_val["inputSchema"].clone(),
                server_name: name.to_string(),
            };
            if !tool.name.is_empty() {
                tools.push(tool);
            }
        }

        Ok(tools)
    }

    /// 调用远程 MCP 工具
    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value> {
        let handle = self.servers.get_mut(server_name).ok_or_else(|| {
            anyhow!("MCP server '{}' not connected", server_name)
        })?;

        // 验证工具存在
        if !handle.tools.iter().any(|t| t.name == tool_name) {
            return Err(anyhow!(
                "tool '{}' not found on MCP server '{}'",
                tool_name,
                server_name
            ));
        }

        let request = json!({
            "jsonrpc": "2.0",
            "id": handle.next_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });
        handle.next_id += 1;

        let response = Self::send_request(handle, &request).await?;

        if let Some(error) = response.get("error") {
            return Err(anyhow!(
                "MCP tool error: {}",
                error["message"].as_str().unwrap_or("unknown error")
            ));
        }

        Ok(response["result"].clone())
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

    /// 断开指定服务器
    pub async fn disconnect(&mut self, name: &str) -> Result<()> {
        if let Some(mut handle) = self.servers.remove(name) {
            let _ = handle.child.kill().await;
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

    /// 发送 JSON-RPC 请求并读取响应
    async fn send_request(handle: &mut McpServerHandle, request: &Value) -> Result<Value> {
        let mut line = serde_json::to_string(request)?;
        line.push('\n');

        handle.stdin.write_all(line.as_bytes()).await?;
        handle.stdin.flush().await?;

        let mut response_line = String::new();
        let reader = handle.stdout.clone();
        let mut locked = reader.lock().await;

        // 超时读取
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            locked.read_line(&mut response_line),
        )
        .await
        {
            Ok(Ok(0)) => Err(anyhow!("MCP server closed connection")),
            Ok(Ok(_)) => {
                let response: Value = serde_json::from_str(response_line.trim())?;
                Ok(response)
            }
            Ok(Err(e)) => Err(anyhow!("read error: {}", e)),
            Err(_) => Err(anyhow!("MCP server response timeout")),
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
        assert_eq!(
            McpClientManager::resolve_env_var("${TEST_MCP_VAR}"),
            "resolved_value"
        );
        assert_eq!(
            McpClientManager::resolve_env_var("literal"),
            "literal"
        );
        std::env::remove_var("TEST_MCP_VAR");
    }

    #[test]
    fn test_mcp_client_manager_new() {
        let manager = McpClientManager::new();
        assert!(manager.all_tools().is_empty());
        assert!(manager.connected_servers().is_empty());
    }
}
