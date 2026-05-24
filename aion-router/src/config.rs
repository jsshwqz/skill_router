//! 统一环境变量配置管理
//!
//! 所有 `std::env::var` 调用都应通过此模块读取，
//! 避免魔法字符串散落在各处，也方便未来迁移到配置文件。

use std::{env, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiProtocol {
    OpenAiChat,
    AnthropicMessages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiEndpoint {
    pub label: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub protocol: AiProtocol,
}

impl AiEndpoint {
    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    pub fn anthropic_messages_url(&self) -> String {
        format!("{}/messages", self.base_url.trim_end_matches('/'))
    }
}

fn env_value(name: &str) -> Option<String> {
    env_file_value(name)
        .or_else(|| env::var(name).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_file_value(name: &str) -> Option<String> {
    let home = env::var("USERPROFILE").or_else(|_| env::var("HOME")).ok()?;
    let path = Path::new(&home).join(".aion").join(".env");
    let content = std::fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim() == name {
            Some(value.trim().trim_matches('"').trim_matches('\'').to_string())
        } else {
            None
        }
    })
}

// ── AI 推理后端 ─────────────────────────────────────────────────────────────

/// Ollama / 本地 LLM 的 API Base URL
pub fn ai_base_url() -> String {
    env_value("AI_BASE_URL").unwrap_or_else(|| "http://localhost:11434/v1".to_string())
}

/// AI API Key（Ollama 默认为 "ollama"）
pub fn ai_api_key() -> String {
    env_value("AI_API_KEY").unwrap_or_else(|| "ollama".to_string())
}

/// 默认使用的模型名称
pub fn ai_model() -> String {
    env_value("AI_MODEL").unwrap_or_else(|| "qwen2.5:7b".to_string())
}

// ── OpenAI ──────────────────────────────────────────────────────────────────

pub fn openai_base_url() -> String {
    env_value("OPENAI_BASE_URL").unwrap_or_else(|| "https://api.openai.com/v1".to_string())
}

pub fn openai_api_key() -> String {
    env_value("OPENAI_API_KEY").unwrap_or_default()
}

pub fn openai_model() -> String {
    env_value("OPENAI_MODEL").unwrap_or_else(|| "gpt-4o".to_string())
}

// ── Google AI ───────────────────────────────────────────────────────────────

pub fn google_ai_base_url() -> String {
    env_value("GOOGLE_AI_BASE_URL").unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string())
}

pub fn google_ai_api_key() -> String {
    env_value("GOOGLE_AI_API_KEY").unwrap_or_default()
}

pub fn google_ai_model() -> String {
    env_value("GOOGLE_AI_MODEL").unwrap_or_else(|| "gemini-2.0-flash".to_string())
}

pub fn candidate_ai_endpoints() -> Vec<AiEndpoint> {
    let mut endpoints = Vec::new();

    push_anthropic_endpoint(
        &mut endpoints,
        "host-anthropic-proxy",
        env_value("ANTHROPIC_BASE_URL")
            .or_else(|| env_value("AION_HOST_AI_BASE_URL"))
            .or_else(|| Some("http://127.0.0.1:8080/v1".to_string())),
        env_value("ANTHROPIC_API_KEY")
            .or_else(|| env_value("AION_HOST_AI_API_KEY"))
            .or_else(|| Some("host-proxy".to_string())),
        env_value("ANTHROPIC_MODEL")
            .or_else(|| env_value("AION_HOST_AI_MODEL"))
            .or_else(|| Some("deepseek-chat".to_string())),
    );
    push_openai_endpoint(
        &mut endpoints,
        "primary",
        env_value("AI_BASE_URL"),
        env_value("AI_API_KEY"),
        env_value("AI_MODEL"),
        "qwen2.5:7b",
    );
    push_openai_endpoint(
        &mut endpoints,
        "opencode-zen",
        env_value("OPENCODE_ZEN_BASE_URL"),
        env_value("OPENCODE_ZEN_API_KEY"),
        env_value("OPENCODE_ZEN_MODEL"),
        "claude-opus-4-6",
    );
    push_openai_endpoint(
        &mut endpoints,
        "openrouter",
        env_value("OPENROUTER_BASE_URL")
            .or_else(|| env_value("OPENROUTER_API_KEY").map(|_| "https://openrouter.ai/api/v1".to_string())),
        env_value("OPENROUTER_API_KEY"),
        env_value("OPENROUTER_MODEL"),
        "inclusionai/ling-2.6-1t:free",
    );
    push_openai_endpoint(
        &mut endpoints,
        "openai-compatible",
        env_value("OPENAI_BASE_URL"),
        env_value("OPENAI_API_KEY"),
        env_value("OPENAI_MODEL"),
        "gpt-4o",
    );
    push_openai_endpoint(
        &mut endpoints,
        "google-ai-compatible",
        env_value("GOOGLE_AI_BASE_URL"),
        env_value("GOOGLE_AI_API_KEY"),
        env_value("GOOGLE_AI_MODEL"),
        "gemini-2.0-flash",
    );
    push_openai_endpoint(
        &mut endpoints,
        "zhipu-compatible",
        env_value("ZHIPU_AI_BASE_URL"),
        env_value("ZHIPU_AI_API_KEY"),
        env_value("ZHIPU_AI_MODEL"),
        "glm-4.5-air",
    );
    push_openai_endpoint(
        &mut endpoints,
        "ollama-local",
        Some("http://localhost:11434/v1".to_string()),
        Some("ollama".to_string()),
        Some("qwen2.5:7b".to_string()),
        "qwen2.5:7b",
    );

    dedupe_endpoints(endpoints)
}

fn push_openai_endpoint(
    endpoints: &mut Vec<AiEndpoint>,
    label: &str,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    fallback_model: &str,
) {
    let Some(base_url) = base_url else {
        return;
    };
    endpoints.push(AiEndpoint {
        label: label.to_string(),
        base_url,
        api_key: api_key.unwrap_or_default(),
        model: model.unwrap_or_else(|| fallback_model.to_string()),
        protocol: AiProtocol::OpenAiChat,
    });
}

fn push_anthropic_endpoint(
    endpoints: &mut Vec<AiEndpoint>,
    label: &str,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
) {
    let Some(base_url) = base_url else {
        return;
    };
    endpoints.push(AiEndpoint {
        label: label.to_string(),
        base_url,
        api_key: api_key.unwrap_or_default(),
        model: model.unwrap_or_else(|| "deepseek-chat".to_string()),
        protocol: AiProtocol::AnthropicMessages,
    });
}

fn dedupe_endpoints(endpoints: Vec<AiEndpoint>) -> Vec<AiEndpoint> {
    let mut seen = std::collections::HashSet::new();
    endpoints
        .into_iter()
        .filter(|endpoint| {
            seen.insert(format!(
                "{}|{}|{:?}",
                endpoint.base_url.trim_end_matches('/'),
                endpoint.model,
                endpoint.protocol
            ))
        })
        .collect()
}

// ── 搜索服务 ────────────────────────────────────────────────────────────────

/// SerpAPI Key，用于 web_search / discovery_search 能力
pub fn serpapi_key() -> String {
    env_value("SERPAPI_KEY").unwrap_or_default()
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
