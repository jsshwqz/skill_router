//! 适配器配置生成器
//!
//! 从 CapabilityRegistry 自动生成各 AI 平台的适配器配置文件：
//! - Claude MCP (`claude_desktop_config.json`)
//! - OpenAI Functions (`functions.json`)
//! - OpenAPI 3.0 (`openapi.yaml`)

use std::path::Path;

use anyhow::Result;
use serde_json::{json, Value};

use aion_router::SkillRouter;
use aion_types::types::RouterPaths;

/// 生成所有适配器配置
#[allow(dead_code)]
pub fn generate_all(paths: RouterPaths, output_dir: &Path) -> Result<()> {
    let router = SkillRouter::new(paths)?;
    let caps: Vec<_> = router.registry().definitions().cloned().collect();

    std::fs::create_dir_all(output_dir.join("claude-mcp"))?;
    std::fs::create_dir_all(output_dir.join("openai"))?;
    std::fs::create_dir_all(output_dir.join("http"))?;
    std::fs::create_dir_all(output_dir.join("aionui"))?;

    generate_mcp_config(output_dir)?;
    generate_openai_functions(&caps, output_dir)?;
    generate_openapi_spec(&caps, output_dir)?;
    generate_aionui_config(&caps, output_dir)?;

    Ok(())
}

/// 生成 Claude MCP 配置
pub fn generate_mcp_config(output_dir: &Path) -> Result<()> {
    let config = json!({
        "mcpServers": {
            "aion-forge": {
                "command": "aion-cli",
                "args": ["mcp-server"],
                "env": {
                    "AI_BASE_URL": "http://localhost:11434/v1",
                    "AI_MODEL": "qwen2.5:7b"
                }
            }
        }
    });

    let path = output_dir.join("claude-mcp/claude_desktop_config.json");
    std::fs::write(&path, serde_json::to_string_pretty(&config)?)?;
    tracing::info!("Generated: {}", path.display());
    Ok(())
}

/// 生成 OpenAI Function Calling 格式
pub fn generate_openai_functions(
    caps: &[aion_types::capability_registry::CapabilityDefinition],
    output_dir: &Path,
) -> Result<()> {
    let functions: Vec<Value> = caps
        .iter()
        .map(|cap| {
            let schema = if cap.parameters_schema.is_null() || cap.parameters_schema == json!({}) {
                let mut props = serde_json::Map::new();
                for input in &cap.inputs {
                    props.insert(
                        input.clone(),
                        json!({ "type": "string", "description": input }),
                    );
                }
                json!({
                    "type": "object",
                    "properties": Value::Object(props),
                    "required": cap.inputs
                })
            } else {
                cap.parameters_schema.clone()
            };

            json!({
                "type": "function",
                "function": {
                    "name": cap.name,
                    "description": cap.description,
                    "parameters": schema
                }
            })
        })
        .collect();

    let path = output_dir.join("openai/functions.json");
    std::fs::write(&path, serde_json::to_string_pretty(&functions)?)?;
    tracing::info!("Generated: {}", path.display());
    Ok(())
}

/// 生成 OpenAPI 3.0 规范（JSON 格式，方便生成）
pub fn generate_openapi_spec(
    caps: &[aion_types::capability_registry::CapabilityDefinition],
    output_dir: &Path,
) -> Result<()> {
    // 为每个 capability 在 POST /v1/route 的描述中列出
    let capability_list: Vec<String> = caps.iter().map(|c| {
        format!("- **{}**: {}", c.name, c.description)
    }).collect();

    let spec = json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Aion Forge API",
            "description": format!(
                "Universal AI Agent Capability Router — {} built-in skills.\n\n## Available Capabilities\n\n{}",
                caps.len(),
                capability_list.join("\n")
            ),
            "version": env!("CARGO_PKG_VERSION"),
            "license": { "name": "MIT" }
        },
        "servers": [
            { "url": "http://localhost:3000", "description": "Local development" }
        ],
        "paths": {
            "/v1/health": {
                "get": {
                    "summary": "Health check",
                    "operationId": "health",
                    "responses": {
                        "200": {
                            "description": "Service is healthy",
                            "content": { "application/json": { "schema": {
                                "type": "object",
                                "properties": {
                                    "status": { "type": "string", "example": "ok" },
                                    "version": { "type": "string" },
                                    "service": { "type": "string" }
                                }
                            }}}
                        }
                    }
                }
            },
            "/v1/capabilities": {
                "get": {
                    "summary": "List all registered capabilities",
                    "operationId": "listCapabilities",
                    "responses": {
                        "200": {
                            "description": "Array of capability definitions",
                            "content": { "application/json": { "schema": {
                                "type": "array",
                                "items": { "$ref": "#/components/schemas/CapabilityDefinition" }
                            }}}
                        }
                    }
                }
            },
            "/v1/route": {
                "post": {
                    "summary": "Route a natural language task to the appropriate skill",
                    "operationId": "routeTask",
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": {
                            "type": "object",
                            "required": ["task"],
                            "properties": {
                                "task": { "type": "string", "description": "Natural language task description" },
                                "context": { "type": "object", "description": "Optional context parameters" }
                            }
                        }}}
                    },
                    "responses": {
                        "200": {
                            "description": "Task execution result",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/RouteResponse" }}}
                        }
                    }
                }
            },
            "/v1/route/native": {
                "post": {
                    "summary": "Structured Agent-to-Agent task routing",
                    "operationId": "routeNative",
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/AiNativePayload" }}}
                    },
                    "responses": {
                        "200": {
                            "description": "Task execution result",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/RouteResponse" }}}
                        }
                    }
                }
            },
            "/v1/memory/recall": {
                "get": {
                    "summary": "Recall memories by keyword search",
                    "operationId": "memoryRecall",
                    "parameters": [
                        { "name": "query", "in": "query", "required": true, "schema": { "type": "string" }},
                        { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 10 }}
                    ],
                    "responses": {
                        "200": { "description": "Matching memory entries" }
                    }
                }
            },
            "/v1/memory/remember": {
                "post": {
                    "summary": "Store a new memory entry",
                    "operationId": "memoryRemember",
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": {
                            "type": "object",
                            "required": ["category", "content"],
                            "properties": {
                                "category": { "type": "string" },
                                "content": { "type": "string" },
                                "session_id": { "type": "string" },
                                "importance": { "type": "integer", "minimum": 1, "maximum": 10 }
                            }
                        }}}
                    },
                    "responses": {
                        "200": { "description": "Memory entry stored" }
                    }
                }
            },
            "/v1/memory/stats": {
                "get": {
                    "summary": "Get memory storage statistics",
                    "operationId": "memoryStats",
                    "responses": {
                        "200": { "description": "Storage statistics" }
                    }
                }
            },
            "/v1/agents": {
                "get": {
                    "summary": "Get agent node information",
                    "operationId": "getAgents",
                    "responses": {
                        "200": { "description": "Agent node info" }
                    }
                }
            },
            "/v1/agents/delegate": {
                "post": {
                    "summary": "Delegate a task to a specific agent",
                    "operationId": "delegateToAgent",
                    "requestBody": {
                        "required": true,
                        "content": { "application/json": { "schema": { "$ref": "#/components/schemas/AiNativePayload" }}}
                    },
                    "responses": {
                        "200": { "description": "Delegation result" }
                    }
                }
            },
            "/v1/metrics": {
                "get": {
                    "summary": "Prometheus metrics endpoint",
                    "operationId": "metrics",
                    "responses": {
                        "200": { "description": "Prometheus format metrics", "content": { "text/plain": {} } }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "CapabilityDefinition": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "description": { "type": "string" },
                        "inputs": { "type": "array", "items": { "type": "string" } },
                        "outputs": { "type": "array", "items": { "type": "string" } },
                        "parameters_schema": { "type": "object" },
                        "examples": { "type": "array", "items": { "type": "object" } }
                    }
                },
                "AiNativePayload": {
                    "type": "object",
                    "required": ["intent"],
                    "properties": {
                        "intent": { "type": "string", "description": "Capability name or natural language intent" },
                        "capability": { "type": "string", "description": "Direct capability (skip planner)" },
                        "parameters": { "type": "object", "description": "Structured parameters" },
                        "priority": { "type": "string", "enum": ["background", "low", "normal", "high", "critical"], "default": "normal" },
                        "target_agent_id": { "type": "string", "description": "Target agent for delegation" },
                        "metadata": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "session_id": { "type": "string" },
                                "backend": { "type": "string", "enum": ["ollama", "openai", "googleai"] }
                            }
                        }
                    }
                },
                "RouteResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string" },
                        "session_id": { "type": "string" },
                        "capability": { "type": "string" },
                        "skill": { "type": "string" },
                        "execution": {
                            "type": "object",
                            "properties": {
                                "status": { "type": "string" },
                                "result": { "type": "object" },
                                "error": { "type": "string" }
                            }
                        },
                        "lifecycle": { "type": "string" }
                    }
                }
            }
        }
    });

    let path = output_dir.join("http/openapi.json");
    std::fs::write(&path, serde_json::to_string_pretty(&spec)?)?;
    tracing::info!("Generated: {}", path.display());
    Ok(())
}

/// 生成 aionui skill.json 配置
pub fn generate_aionui_config(
    caps: &[aion_types::capability_registry::CapabilityDefinition],
    output_dir: &Path,
) -> Result<()> {
    let capabilities: Vec<String> = caps.iter().map(|c| c.name.clone()).collect();

    let tools: Vec<Value> = caps
        .iter()
        .map(|cap| {
            json!({
                "type": "function",
                "function": {
                    "name": cap.name,
                    "description": cap.description,
                    "parameters": if cap.parameters_schema.is_null() || cap.parameters_schema == json!({}) {
                        let mut props = serde_json::Map::new();
                        for input in &cap.inputs {
                            props.insert(input.clone(), json!({ "type": "string", "description": input }));
                        }
                        json!({ "type": "object", "properties": Value::Object(props), "required": cap.inputs })
                    } else {
                        cap.parameters_schema.clone()
                    }
                }
            })
        })
        .collect();

    let config = json!({
        "name": "aion-forge",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Universal AI Agent Capability Router — 29+ built-in skills",
        "entrypoint": "aion-cli",
        "entrypoint_dev": "cargo run -p aion-cli --",
        "safety_manifest": "safety-manifest.json",
        "permissions": {
            "network": true,
            "filesystem_read": true,
            "filesystem_write": true,
            "process_exec": false
        },
        "capabilities": capabilities,
        "api_schema": { "tools": tools }
    });

    let path = output_dir.join("aionui/skill.json");
    std::fs::write(&path, serde_json::to_string_pretty(&config)?)?;
    tracing::info!("Generated: {}", path.display());
    Ok(())
}
