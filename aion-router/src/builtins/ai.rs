//! AI task builtin skill: ai_task
//! Supports fallback chain: host Anthropic proxy -> configured AI providers -> Ollama local
//! 当 AI_TOON_ENABLED=true 时，自动将输入中的 JSON 数据转为 TOON 格式，节省 token。

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::config::{candidate_ai_endpoints, AiEndpoint, AiProtocol};
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::format;
use super::BuiltinSkill;

/// 是否启用 TOON 自动转换（环境变量控制）
fn toon_enabled() -> bool {
    std::env::var("AI_TOON_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// 从当前工作目录的 CLAUDE.md 或 FORGE_PROJECT_RULES 环境变量加载项目规约，
/// 作为 AI 指令的后缀，使生成/审查的代码符合项目约定。
fn project_rules_suffix() -> String {
    // 环境变量优先（显式覆盖）
    if let Ok(rules) = std::env::var("FORGE_PROJECT_RULES") {
        if !rules.is_empty() {
            return format!("\n\n## Project Rules\n{}\n", rules);
        }
    }

    // 自动检测 CLAUDE.md
    if let Ok(content) = std::fs::read_to_string("CLAUDE.md") {
        let rules: Vec<&str> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| {
                l.starts_with("- ") || l.starts_with("* ") || l.starts_with("|")
            })
            .collect();
        if !rules.is_empty() {
            return format!(
                "\n\n## Project Rules (from CLAUDE.md)\n{}\n",
                rules.join("\n")
            );
        }
    }

    String::new()
}

pub struct AiTask;

impl AiEndpoint {
    /// Check if a provider is disabled via AI_PROVIDERS_DISABLED env var.
    /// Example: AI_PROVIDERS_DISABLED=ollama-local,some-other
    fn is_disabled(label: &str) -> bool {
        std::env::var("AI_PROVIDERS_DISABLED")
            .map(|v| v.split(',').any(|s| s.trim() == label))
            .unwrap_or(false)
    }

    fn enabled_candidates() -> Vec<Self> {
        candidate_ai_endpoints()
            .into_iter()
            .filter(|endpoint| !Self::is_disabled(&endpoint.label))
            .collect()
    }
}

#[async_trait::async_trait]
impl BuiltinSkill for AiTask {
    fn name(&self) -> &'static str {
        "ai_task"
    }

    async fn execute(&self, skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let base_instruction = skill.metadata.instruction.as_deref().ok_or_else(|| {
            anyhow!(
                "skill '{}' uses builtin:ai_task but has no 'instruction' field in skill.json",
                skill.metadata.name
            )
        })?;

        // 追加项目规约（来自 CLAUDE.md 或 FORGE_PROJECT_RULES）
        let instruction = format!("{}{}", base_instruction, project_rules_suffix());

        let text = context.context["text"]
            .as_str()
            .or_else(|| context.context["input"].as_str())
            .unwrap_or(&context.task)
            .to_string();

        // AI_TOON_ENABLED 时，自动将 JSON 数据转 TOON 格式节省 token
        let text = if toon_enabled() {
            maybe_toon(&text)
        } else {
            text
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        let endpoints = AiEndpoint::enabled_candidates();
        let mut last_error = String::new();

        for ep in &endpoints {
            tracing::info!("ai_task trying [{}] {} model={}", ep.label, ep.base_url, ep.model);

            match run_endpoint(&client, ep, &instruction, &text).await {
                Ok(content) => {
                    tracing::info!("ai_task [{}] success, output len={}", ep.label, content.len());
                    return Ok(json!({
                        "task": context.task,
                        "capability": context.capability,
                        "output": content,
                        "provider": ep.label,
                    }));
                }
                Err(e) => {
                    last_error = format!("[{}] {}", ep.label, e);
                    tracing::warn!("ai_task [{}] failed: {}", ep.label, e);
                }
            }
        }

        // All providers failed — return error with details (not silent empty output)
        Ok(json!({
            "task": context.task,
            "capability": context.capability,
            "output": format!("AI service unavailable. Last error: {}", last_error),
            "error": last_error,
        }))
    }
}

/// 尝试将 JSON 文本转换为 TOON 格式，失败则返回原文
fn maybe_toon(text: &str) -> String {
    // 只对明显是 JSON 的文本做转换（以 { 或 [ 开头）
    let trimmed = text.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return text.to_string();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => {
            let toon = format::json_to_toon(&val, 0);
            let chars_saved = trimmed.len().saturating_sub(toon.len());
            if chars_saved > 0 {
                tracing::info!("TOON: saved ~{} chars ({:.0}%)", chars_saved,
                    chars_saved as f64 / trimmed.len() as f64 * 100.0);
            }
            // 如果是纯 JSON 数据（指令里套 JSON），加 TOON 格式提示
            format!("(以下数据使用 TOON 格式，类似 YAML 但数组用 [N]{{fields}}: 表示)\n{}", toon)
        }
        Err(_) => text.to_string(), // 不是有效 JSON，返回原文
    }
}

async fn run_endpoint(
    client: &reqwest::Client,
    endpoint: &AiEndpoint,
    instruction: &str,
    text: &str,
) -> Result<String> {
    match endpoint.protocol {
        AiProtocol::OpenAiChat => run_openai_chat(client, endpoint, instruction, text).await,
        AiProtocol::AnthropicMessages => run_anthropic_messages(client, endpoint, instruction, text).await,
    }
}

async fn run_openai_chat(
    client: &reqwest::Client,
    endpoint: &AiEndpoint,
    instruction: &str,
    text: &str,
) -> Result<String> {
    // 系统指令：开启 prompt caching 支持（OpenRouter 兼容）
    // 缓存后重复调用仅收 10% 费用
    let sys_msg = json!({
        "role": "system",
        "content": instruction,
        "cache_control": {"type": "ephemeral"}
    });

    let body = json!({
        "model": endpoint.model,
        "messages": [
            sys_msg,
            {"role": "user", "content": text}
        ],
        "max_tokens": 512,
        "temperature": 0.3
    });

    let resp = client
        .post(endpoint.chat_completions_url())
        .header("Authorization", format!("Bearer {}", endpoint.api_key))
        .json(&body)
        .send()
        .await?;
    parse_openai_response(resp).await
}

async fn run_anthropic_messages(
    client: &reqwest::Client,
    endpoint: &AiEndpoint,
    instruction: &str,
    text: &str,
) -> Result<String> {
    let body = json!({
        "model": endpoint.model,
        "system": [{"type": "text", "text": instruction, "cache_control": {"type": "ephemeral"}}],
        "max_tokens": 2048,
        "messages": [{"role": "user", "content": text}],
        "stream": false
    });

    let resp = client
        .post(endpoint.anthropic_messages_url())
        .header("x-api-key", &endpoint.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31")
        .json(&body)
        .send()
        .await?;
    parse_anthropic_response(resp).await
}

async fn parse_openai_response(resp: reqwest::Response) -> Result<String> {
    let status = resp.status();
    let raw = resp.text().await.unwrap_or_default();
    let parsed: Value = serde_json::from_str(&raw).unwrap_or_default();
    if !status.is_success() {
        let err_msg = parsed["error"]["message"]
            .as_str()
            .or_else(|| parsed["message"].as_str())
            .unwrap_or("AI backend returned an error");
        bail!("{err_msg}");
    }
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .or_else(|| parsed["choices"][0]["delta"]["content"].as_str())
        .or_else(|| parsed["result"].as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    validate_content(content)
}

async fn parse_anthropic_response(resp: reqwest::Response) -> Result<String> {
    let status = resp.status();
    let raw = resp.text().await.unwrap_or_default();
    let parsed: Value = serde_json::from_str(&raw).unwrap_or_default();
    if !status.is_success() {
        let err_msg = parsed["error"]["message"]
            .as_str()
            .or_else(|| parsed["message"].as_str())
            .unwrap_or("AI backend returned an error");
        bail!("{err_msg}");
    }
    let content = parsed["content"][0]["text"].as_str().unwrap_or("").trim().to_string();
    validate_content(content)
}

fn validate_content(content: String) -> Result<String> {
    if content.is_empty() {
        bail!("AI backend returned empty content");
    }
    if matches!(content.as_str(), "参数错误" | "Invalid API key.") {
        bail!("AI backend returned provider error text: {content}");
    }
    Ok(content)
}
