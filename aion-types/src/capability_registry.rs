use std::{collections::BTreeMap, fs};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::RouterPaths;

// ── Registry Backend Trait ──────────────────────────────────────────────────

/// 能力注册表的后端抽象
///
/// 允许切换不同存储后端（本地 BTreeMap、NATS JetStream KV、混合模式）
/// 而不影响上层 `CapabilityRegistry` 的公开 API。
///
/// # 迁移路径
/// - MVP：`CapabilityRegistry`（即 `LocalRegistryBackend`，BTreeMap）
/// - Phase 1：`HybridRegistryBackend`（本地缓存 + NATS 远程）
/// - Phase 2：`NatsRegistryBackend`（纯 NATS JetStream KV）
pub trait RegistryBackend: Send + Sync {
    /// 查找指定名称的能力定义
    fn get(&self, name: &str) -> Option<CapabilityDefinition>;
    /// 注册或更新一个能力定义
    fn put(&mut self, def: CapabilityDefinition) -> Result<()>;
    /// 列出所有已注册的能力
    fn list(&self) -> Vec<CapabilityDefinition>;
    /// 检查是否包含指定能力
    fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
    /// 已注册能力的数量
    fn len(&self) -> usize {
        self.list().len()
    }
    /// 是否为空
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDefinition {
    pub name: String,
    pub description: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub parameters_schema: Value,
    #[serde(default)]
    pub examples: Vec<Value>,
}

impl CapabilityDefinition {
    /// Whether this capability requires an AI/LLM backend to execute.
    /// Determined by convention: description contains "using AI" or "using LLM".
    pub fn requires_ai(&self) -> bool {
        let d = self.description.to_lowercase();
        d.contains("using ai") || d.contains("using llm")
    }

    /// Whether this capability requires network access.
    pub fn requires_network(&self) -> bool {
        self.requires_ai()
            || self.name.contains("search")
            || self.name.contains("fetch")
            || self.name == "discovery_search"
    }
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    definitions: BTreeMap<String, CapabilityDefinition>,
}

impl CapabilityRegistry {
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        for definition in [
            CapabilityDefinition {
                name: "yaml_parse".to_string(),
                description: "Parse YAML text into structured JSON data".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["parsed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "The YAML content to parse" }
                    },
                    "required": ["text"]
                }),
                examples: vec![serde_json::json!({
                    "intent": "yaml_parse",
                    "parameters": { "text": "foo: bar\nlist:\n  - 1\n  - 2" }
                })],
            },
            CapabilityDefinition {
                name: "json_parse".to_string(),
                description: "Parse and validate JSON text into structured data".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["parsed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "toml_parse".to_string(),
                description: "Parse TOML configuration text into structured data".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["parsed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "csv_parse".to_string(),
                description: "Parse CSV or spreadsheet text into rows and columns".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["rows".to_string(), "headers".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "pdf_parse".to_string(),
                description: "Extract and structure text content from a PDF file path".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["structured_data".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "markdown_render".to_string(),
                description: "Parse Markdown text into structured sections".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["sections".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_summarize".to_string(),
                description: "Summarize text using AI into a concise output".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_translate".to_string(),
                description: "Translate text from one language to another using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_classify".to_string(),
                description: "Classify or categorize text into a label using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_extract".to_string(),
                description: "Extract key entities and information from text using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_diff".to_string(),
                description: "Compute a line-level diff between two text inputs".to_string(),
                inputs: vec!["a".to_string(), "b".to_string()],
                outputs: vec!["diff".to_string(), "added".to_string(), "removed".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "a": { "type": "string" }, "b": { "type": "string" } },
                    "required": ["a", "b"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "text_embed".to_string(),
                description: "Compute a term-frequency bag-of-words vector for text".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["vector".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "web_search".to_string(),
                description: "Search the web via SerpAPI and return organic results".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["results".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "http_fetch".to_string(),
                description: "Fetch the body of an HTTPS URL".to_string(),
                inputs: vec!["url".to_string()],
                outputs: vec!["body".to_string(), "status".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "url": { "type": "string" } },
                    "required": ["url"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "image_describe".to_string(),
                description: "Describe an image at a given path or URL using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "code_generate".to_string(),
                description: "Generate Rust code for a given requirement using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "code_test".to_string(),
                description: "Write Rust unit tests for given code using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "code_lint".to_string(),
                description: "Review Rust code for issues and suggest fixes using AI".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "discovery_search".to_string(),
                description: "Cascade search across Google, HTTP fallback, and local trusted sources".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["hits".to_string(), "sources_succeeded".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_remember".to_string(),
                description: "Persist a memory entry (decision, lesson, error, preference, etc.) to long-term store".to_string(),
                inputs: vec!["content".to_string(), "category".to_string()],
                outputs: vec!["memory_id".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "content": { "type": "string" }, "category": { "type": "string" } },
                    "required": ["content"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_recall".to_string(),
                description: "Recall relevant memories by keyword search from long-term store".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["results".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_distill".to_string(),
                description: "Distill and compact the memory store by removing duplicates and decaying old entries".to_string(),
                inputs: vec![],
                outputs: vec!["removed".to_string(), "merged".to_string()],
                parameters_schema: serde_json::json!({ "type": "object" }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "echo".to_string(),
                description: "Simply echo back the input task for testing".to_string(),
                inputs: vec!["text".to_string()],
                outputs: vec!["output".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "space_navigation".to_string(),
                description: "Navigate to interstellar destinations (experimental)".to_string(),
                inputs: vec!["destination".to_string()],
                outputs: vec!["status".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "destination": { "type": "string" } },
                    "required": ["destination"]
                }),
                examples: vec![],
            },
            // ── Spec-Driven 规格驱动开发 ─────────────────────────────────────
            CapabilityDefinition {
                name: "spec_driven".to_string(),
                description: "Structured 5-phase pipeline for large-scale code transformations: analyze, decompose, plan, execute, learn. Forces disciplined approach instead of immediate coding.".to_string(),
                inputs: vec!["action".to_string()],
                outputs: vec!["project_id".to_string(), "status".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["analyze","decompose","plan","execute","status"], "description": "Pipeline phase to invoke" },
                        "goal": { "type": "string", "description": "Project goal (required for analyze)" },
                        "project_id": { "type": "string", "description": "Project ID (required for decompose/plan/execute/status)" },
                        "workspace": { "type": "string", "description": "Workspace root path (optional)" },
                        "task_id": { "type": "string", "description": "Sub-task ID (for execute)" },
                        "task_result": { "type": "string", "description": "Submit execution result (for execute)" },
                        "task_error": { "type": "string", "description": "Report task failure (for execute)" },
                        "analysis_result": { "type": "object", "description": "Analysis output (for analyze callback)" },
                        "tasks": { "type": "array", "description": "Task list (for decompose callback)" },
                        "plan_result": { "type": "object", "description": "Plan output (for plan callback)" }
                    },
                    "required": ["action"]
                }),
                examples: vec![],
            },
            // ── Agent 专项能力（MA4）──────────────────────────────────────────
            CapabilityDefinition {
                name: "agent_delegate".to_string(),
                description: "Delegate a task to a specific Agent by ID via the message bus".to_string(),
                inputs: vec!["task".to_string(), "target_agent_id".to_string()],
                outputs: vec!["result".to_string(), "delegation_chain".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "The task to delegate" },
                        "target_agent_id": { "type": "string", "description": "Target Agent ID" },
                        "capability": { "type": "string", "description": "Capability to invoke on target" }
                    },
                    "required": ["task", "target_agent_id"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "agent_broadcast".to_string(),
                description: "Broadcast a message to all registered Agents on the message bus".to_string(),
                inputs: vec!["message".to_string()],
                outputs: vec!["delivered_count".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string", "description": "Message content to broadcast" }
                    },
                    "required": ["message"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "agent_gather".to_string(),
                description: "Send a query to multiple Agents and aggregate all responses".to_string(),
                inputs: vec!["query".to_string(), "agent_ids".to_string()],
                outputs: vec!["responses".to_string(), "success_rate".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Query to send to Agents" },
                        "agent_ids": { "type": "array", "items": { "type": "string" }, "description": "Target Agent IDs" },
                        "timeout_secs": { "type": "integer", "default": 10 }
                    },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "agent_status".to_string(),
                description: "Query the status of all registered Agents or a specific Agent".to_string(),
                inputs: vec![],
                outputs: vec!["agents".to_string(), "total".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "agent_id": { "type": "string", "description": "Optional specific Agent ID to query" }
                    }
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "task_pipeline".to_string(),
                description: "Execute a serial pipeline of capabilities, passing each result to the next step".to_string(),
                inputs: vec!["steps".to_string(), "initial_input".to_string()],
                outputs: vec!["final_result".to_string(), "step_results".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "steps": { "type": "array", "items": { "type": "string" }, "description": "Ordered list of capability names" },
                        "initial_input": { "type": "string", "description": "Input for the first step" }
                    },
                    "required": ["steps", "initial_input"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "task_race".to_string(),
                description: "Race multiple Agents on the same task, return the first successful result".to_string(),
                inputs: vec!["task".to_string()],
                outputs: vec!["winner_agent".to_string(), "result".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "Task to race" },
                        "agent_ids": { "type": "array", "items": { "type": "string" }, "description": "Competing Agent IDs" },
                        "capability": { "type": "string", "description": "Capability to invoke" }
                    },
                    "required": ["task"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "memory_team_share".to_string(),
                description: "Promote a private memory entry to team-shared namespace for cross-Agent access".to_string(),
                inputs: vec!["memory_id".to_string()],
                outputs: vec!["shared_id".to_string(), "namespace".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "memory_id": { "type": "string", "description": "Private memory ID to share" },
                        "team_session_id": { "type": "string", "description": "Target team session" }
                    },
                    "required": ["memory_id"]
                }),
                examples: vec![],
            },
            // ── F12: 新增技能 ───────────────────────────────────────────
            CapabilityDefinition {
                name: "json_query".to_string(),
                description: "Query JSON data using JSONPath-like expressions ($.key.subkey[0])".to_string(),
                inputs: vec!["data".to_string(), "path".to_string()],
                outputs: vec!["matches".to_string(), "count".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "data": { "type": "string", "description": "JSON string to query" },
                        "path": { "type": "string", "description": "JSONPath expression (e.g. $.store.book[0].title)" }
                    },
                    "required": ["data", "path"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "regex_match".to_string(),
                description: "Match text against a regular expression pattern with find_all, is_match, or captures mode".to_string(),
                inputs: vec!["text".to_string(), "pattern".to_string()],
                outputs: vec!["matches".to_string(), "count".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Text to search" },
                        "pattern": { "type": "string", "description": "Regular expression pattern" },
                        "mode": { "type": "string", "enum": ["find_all", "is_match", "captures"], "default": "find_all" }
                    },
                    "required": ["text", "pattern"]
                }),
                examples: vec![],
            },
            // ── MCP Client ───────────────────────────────────────────────
            CapabilityDefinition {
                name: "mcp_call".to_string(),
                description: "Call an external tool via MCP (Model Context Protocol) server".to_string(),
                inputs: vec!["server".to_string(), "tool".to_string(), "arguments".to_string()],
                outputs: vec!["result".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "server": { "type": "string", "description": "MCP server name" },
                        "tool": { "type": "string", "description": "Tool name on the MCP server" },
                        "arguments": { "type": "object", "description": "Arguments for the tool" }
                    },
                    "required": ["server", "tool"]
                }),
                examples: vec![],
            },
            // ── RAG 检索增强 ─────────────────────────────────────────────
            CapabilityDefinition {
                name: "rag_ingest".to_string(),
                description: "Ingest a document into the RAG knowledge base for later retrieval".to_string(),
                inputs: vec!["source".to_string()],
                outputs: vec!["chunks_added".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "File path or URL to ingest" },
                        "content": { "type": "string", "description": "Optional: document content (if not reading from file)" }
                    },
                    "required": ["source"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "rag_query".to_string(),
                description: "Query the RAG knowledge base and generate an AI-enhanced answer from retrieved documents".to_string(),
                inputs: vec!["query".to_string()],
                outputs: vec!["answer".to_string(), "sources".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Question to ask the knowledge base" },
                        "top_k": { "type": "integer", "description": "Number of relevant chunks to retrieve (default: 3)" }
                    },
                    "required": ["query"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "rag_status".to_string(),
                description: "Show the current status of the RAG knowledge base (document count, chunk count, sources)".to_string(),
                inputs: vec![],
                outputs: vec!["document_count".to_string(), "chunk_count".to_string(), "sources".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                examples: vec![],
            },
            // ── 多模型编排 ──
            CapabilityDefinition {
                name: "ai_smart_collaborate".to_string(),
                description: "Let OpenAI, Claude, Gemini collaborate on a task: discuss, detect consensus, merge or pick best using AI".to_string(),
                inputs: vec!["task".to_string()],
                outputs: vec!["proposals".to_string(), "final_solution".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "Task description, as detailed as possible" },
                        "execute_on_disagreement": { "type": "boolean", "description": "Execute when disagreement (default: true)" }
                    },
                    "required": ["task"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_code_generate".to_string(),
                description: "Generate code with one AI engine and review with another using AI".to_string(),
                inputs: vec!["task".to_string()],
                outputs: vec!["original_code".to_string(), "review".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "Code requirement" },
                        "language": { "type": "string", "description": "Programming language (default: python)" },
                        "primary": { "type": "string", "enum": ["claude", "openai", "gemini"], "description": "Primary generator" },
                        "reviewer": { "type": "string", "enum": ["claude", "openai", "gemini"], "description": "Code reviewer" }
                    },
                    "required": ["task"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_triangle_review".to_string(),
                description: "Three AI engines review code simultaneously, merge opinions with confidence using AI".to_string(),
                inputs: vec!["code".to_string()],
                outputs: vec!["merged_review".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string", "description": "Code to review" },
                        "context": { "type": "string", "description": "Optional code background" }
                    },
                    "required": ["code"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_triple_vote".to_string(),
                description: "Three AI engines independently vote on a problem, pick the best using AI".to_string(),
                inputs: vec!["problem".to_string()],
                outputs: vec!["votes".to_string(), "tally".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "problem": { "type": "string", "description": "Problem description" },
                        "options": { "type": "array", "items": { "type": "string" }, "description": "Predefined options (optional)" }
                    },
                    "required": ["problem"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_parallel_solve".to_string(),
                description: "Multiple AI engines solve a problem in parallel, compare and pick best using AI".to_string(),
                inputs: vec!["problem".to_string()],
                outputs: vec!["solutions".to_string(), "comparison".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "problem": { "type": "string", "description": "Problem description" },
                        "engines": { "type": "array", "items": { "type": "string", "enum": ["claude", "openai", "gemini"] }, "description": "Engines to use (default: all)" }
                    },
                    "required": ["problem"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_serial_optimize".to_string(),
                description: "Configurable AI pipeline to optimize code: analyze, optimize, verify using AI".to_string(),
                inputs: vec!["code".to_string()],
                outputs: vec!["optimized_code".to_string(), "verification".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string", "description": "Code to optimize" },
                        "goals": { "type": "array", "items": { "type": "string" }, "description": "Optimization goals" },
                        "pipeline": { "type": "array", "items": { "type": "string", "enum": ["claude", "openai", "gemini"] }, "description": "AI execution order [analyzer, optimizer, verifier]" }
                    },
                    "required": ["code"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_research".to_string(),
                description: "Three-engine multi-perspective research: Claude theory, OpenAI practice, Gemini trends using AI".to_string(),
                inputs: vec!["topic".to_string()],
                outputs: vec!["synthesis".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "topic": { "type": "string", "description": "Research topic" },
                        "depth": { "type": "string", "enum": ["quick", "comprehensive", "deep"], "description": "Research depth" }
                    },
                    "required": ["topic"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_long_context".to_string(),
                description: "Process long content with Gemini's ultra-long context, optionally verify with other AI".to_string(),
                inputs: vec!["content".to_string(), "task".to_string()],
                outputs: vec!["gemini_analysis".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "content": { "type": "string", "description": "Long text content" },
                        "task": { "type": "string", "description": "Processing task" },
                        "verify_with": { "type": "array", "items": { "type": "string", "enum": ["claude", "openai"] }, "description": "Verification engines (optional)" }
                    },
                    "required": ["content", "task"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "ai_cross_review".to_string(),
                description: "Two AI engines cross-review code, merge opinions using AI".to_string(),
                inputs: vec!["code".to_string()],
                outputs: vec!["merged_review".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string", "description": "Code to review" },
                        "engines": { "type": "array", "items": { "type": "string", "enum": ["claude", "openai", "gemini"] }, "description": "Two engines (default: [claude, openai])" }
                    },
                    "required": ["code"]
                }),
                examples: vec![],
            },
            CapabilityDefinition {
                name: "skill_report".to_string(),
                description: "Generate a report of skill usage statistics from the learning engine".to_string(),
                inputs: vec![],
                outputs: vec!["report".to_string()],
                parameters_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                examples: vec![],
            },
        ] {
            registry
                .definitions
                .insert(definition.name.clone(), definition);
        }
        registry
    }

    pub fn load_or_builtin(paths: &RouterPaths) -> Result<Self> {
        let mut registry = Self::builtin();
        if !paths.capabilities_dir.exists() {
            return Ok(registry);
        }

        for entry in fs::read_dir(&paths.capabilities_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            {
                let definition: CapabilityDefinition =
                    serde_json::from_slice(&fs::read(entry.path())?)?;
                registry.validate_name(&definition.name)?;
                registry
                    .definitions
                    .insert(definition.name.clone(), definition);
            }
        }

        Ok(registry)
    }

    pub fn validate_name(&self, name: &str) -> Result<()> {
        let is_valid = !name.is_empty()
            && !name.starts_with('_')
            && !name.ends_with('_')
            && name
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');

        if is_valid {
            Ok(())
        } else {
            Err(anyhow!("invalid capability name: {name}"))
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &CapabilityDefinition> {
        self.definitions.values()
    }

    /// Write a newly discovered capability to capabilities/ dir so it survives restarts.
    pub fn persist_discovered(&self, name: &str, task: &str) -> anyhow::Result<()> {
        // We don't have paths here, so write to a temp location the caller can move.
        // Instead, callers should use persist_to_dir directly.
        let _ = (name, task);
        Ok(())
    }

    /// 注册一个能力定义（通用接口，等同于 `persist_to_dir` 但不写磁盘）
    pub fn register(&mut self, def: CapabilityDefinition) {
        self.definitions.insert(def.name.clone(), def);
    }

    /// 获取指定名称的能力定义
    pub fn get(&self, name: &str) -> Option<&CapabilityDefinition> {
        self.definitions.get(name)
    }

    /// 已注册能力的数量
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Check if a capability requires AI (by looking at its definition metadata).
    pub fn capability_requires_ai(&self, name: &str) -> bool {
        self.definitions.get(name).map(|d| d.requires_ai()).unwrap_or(false)
    }

    /// Check if a capability requires network access.
    pub fn capability_requires_network(&self, name: &str) -> bool {
        self.definitions.get(name).map(|d| d.requires_network()).unwrap_or(false)
    }

    pub fn persist_to_dir(&mut self, name: &str, task: &str, capabilities_dir: &std::path::Path) -> anyhow::Result<()> {
        if self.contains(name) { return Ok(()); }
        std::fs::create_dir_all(capabilities_dir)?;
        let def = CapabilityDefinition {
            name: name.to_string(),
            description: format!("Auto-discovered capability for: {}", task),
            inputs: vec!["text".to_string()],
            outputs: vec!["output".to_string()],
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                }
            }),
            examples: vec![],
        };
        std::fs::write(
            capabilities_dir.join(format!("{}.json", name)),
            serde_json::to_vec_pretty(&def)?,
        )?;
        self.definitions.insert(name.to_string(), def);
        Ok(())
    }
}

// ── RegistryBackend 实现 ────────────────────────────────────────────────────

impl RegistryBackend for CapabilityRegistry {
    fn get(&self, name: &str) -> Option<CapabilityDefinition> {
        self.definitions.get(name).cloned()
    }

    fn put(&mut self, def: CapabilityDefinition) -> Result<()> {
        self.definitions.insert(def.name.clone(), def);
        Ok(())
    }

    fn list(&self) -> Vec<CapabilityDefinition> {
        self.definitions.values().cloned().collect()
    }

    fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    fn len(&self) -> usize {
        self.definitions.len()
    }
}
