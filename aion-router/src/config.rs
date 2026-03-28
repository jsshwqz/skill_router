//! 统一环境变量配置管理
//!
//! 所有 `std::env::var` 调用都应通过此模块读取，
//! 避免魔法字符串散落在各处，也方便未来迁移到配置文件。

use std::env;

// ── AI 推理后端 ─────────────────────────────────────────────────────────────

/// Ollama / 本地 LLM 的 API Base URL
pub fn ai_base_url() -> String {
    env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string())
}

/// AI API Key（Ollama 默认为 "ollama"）
pub fn ai_api_key() -> String {
    env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string())
}

/// 默认使用的模型名称
pub fn ai_model() -> String {
    env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string())
}

// ── OpenAI ──────────────────────────────────────────────────────────────────

pub fn openai_base_url() -> String {
    env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
}

pub fn openai_api_key() -> String {
    env::var("OPENAI_API_KEY").unwrap_or_default()
}

pub fn openai_model() -> String {
    env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string())
}

// ── Google AI ───────────────────────────────────────────────────────────────

pub fn google_ai_base_url() -> String {
    env::var("GOOGLE_AI_BASE_URL")
        .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".to_string())
}

pub fn google_ai_api_key() -> String {
    env::var("GOOGLE_AI_API_KEY").unwrap_or_default()
}

pub fn google_ai_model() -> String {
    env::var("GOOGLE_AI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".to_string())
}

// ── 搜索服务 ────────────────────────────────────────────────────────────────

/// SerpAPI Key，用于 web_search / discovery_search 能力
pub fn serpapi_key() -> String {
    env::var("SERPAPI_KEY").unwrap_or_default()
}

// ── 安全策略 ─────────────────────────────────────────────────────────────────

/// AI 安全审查失败时的策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityFailPolicy {
    /// 放行（开发环境默认）
    Open,
    /// 拒绝（生产环境推荐）
    Closed,
}

pub fn security_fail_policy() -> SecurityFailPolicy {
    match env::var("AI_SECURITY_FAIL_POLICY")
        .unwrap_or_else(|_| "open".to_string())
        .to_lowercase()
        .as_str()
    {
        "closed" => SecurityFailPolicy::Closed,
        _ => SecurityFailPolicy::Open,
    }
}

// ── 分布式 / 多 Agent ────────────────────────────────────────────────────────

/// NATS 服务地址（None 表示使用进程内消息总线）
pub fn nats_url() -> Option<String> {
    env::var("NATS_URL").ok().filter(|s| !s.is_empty())
}

/// 当前节点角色（orchestrator | planner | executor | specialist | reviewer | memory_keeper）
pub fn node_role() -> String {
    env::var("NODE_ROLE").unwrap_or_else(|_| "orchestrator".to_string())
}

/// 当前节点专属能力列表（specialist 角色时使用）
pub fn node_capabilities() -> Vec<String> {
    env::var("NODE_CAPABILITIES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ── Agent 委派安全 ────────────────────────────────────────────────────────────

/// delegation_chain 最大深度，防止循环委派
pub fn max_delegation_depth() -> usize {
    env::var("MAX_DELEGATION_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}
