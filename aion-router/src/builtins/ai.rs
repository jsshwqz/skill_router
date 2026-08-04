//! AI task builtin skill: ai_task
//! Supports fallback chain: host Anthropic proxy -> configured AI providers -> Ollama local
//! 褰?AI_TOON_ENABLED=true 鏃讹紝鑷姩灏嗚緭鍏ヤ腑鐨?JSON 鏁版嵁杞负 TOON 鏍煎紡锛岃妭鐪?token銆?

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::config::{candidate_ai_endpoints, AiEndpoint, AiProtocol};
use aion_types::types::{ExecutionContext, SkillDefinition, TokenUsage};

use super::format;
use super::BuiltinSkill;

const AI_TASK_MAX_OUTPUT_TOKENS: u64 = 2048;

/// 鏄惁鍚敤 TOON 鑷姩杞崲锛堢幆澧冨彉閲忔帶鍒讹級
fn toon_enabled() -> bool {
    std::env::var("AI_TOON_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// 浠庡綋鍓嶅伐浣滅洰褰曠殑 CLAUDE.md 鎴?FORGE_PROJECT_RULES 鐜鍙橀噺鍔犺浇椤圭洰瑙勭害锛?
/// 浣滀负 AI 鎸囦护鐨勫悗缂€锛屼娇鐢熸垚/瀹℃煡鐨勪唬鐮佺鍚堥」鐩害瀹氥€?
fn project_rules_suffix() -> String {
    // 鐜鍙橀噺浼樺厛锛堟樉寮忚鐩栵級
    if let Ok(rules) = std::env::var("FORGE_PROJECT_RULES") {
        if !rules.is_empty() {
            return format!("\n\n## Project Rules\n{}\n", rules);
        }
    }

    // 鑷姩妫€娴?CLAUDE.md
    if let Ok(content) = std::fs::read_to_string("CLAUDE.md") {
        let rules: Vec<&str> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.starts_with("- ") || l.starts_with("* ") || l.starts_with("|"))
            .collect();
        if !rules.is_empty() {
            return format!("\n\n## Project Rules (from CLAUDE.md)\n{}\n", rules.join("\n"));
        }
    }

    String::new()
}

/// 8-step PromptBuilder for structured AI task execution
/// Steps: role -> context -> rules -> examples -> format -> task -> constraints -> output
pub struct PromptBuilder {
    role: String,
    context: String,
    rules: Vec<String>,
    examples: Vec<String>,
    format: String,
    constraints: Vec<String>,
    output_spec: String,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self {
            role: String::new(),
            context: String::new(),
            rules: Vec::new(),
            examples: Vec::new(),
            format: String::new(),
            constraints: Vec::new(),
            output_spec: String::new(),
        }
    }

    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
    }

    pub fn with_context(mut self, context: &str) -> Self {
        if !context.is_empty() {
            self.context = context.to_string();
        }
        self
    }

    pub fn with_rule(mut self, rule: &str) -> Self {
        if !rule.is_empty() {
            self.rules.push(rule.to_string());
        }
        self
    }

    pub fn with_example(mut self, example: &str) -> Self {
        if !example.is_empty() {
            self.examples.push(example.to_string());
        }
        self
    }

    pub fn with_format(mut self, format: &str) -> Self {
        self.format = format.to_string();
        self
    }

    pub fn with_constraint(mut self, constraint: &str) -> Self {
        if !constraint.is_empty() {
            self.constraints.push(constraint.to_string());
        }
        self
    }

    pub fn with_output(mut self, output: &str) -> Self {
        self.output_spec = output.to_string();
        self
    }

    pub fn build(&self, task: &str) -> String {
        let mut parts = Vec::new();

        if !self.role.is_empty() {
            parts.push(format!("## Role\n{}\n", self.role));
        }

        if !self.context.is_empty() {
            parts.push(format!("## Context\n{}\n", self.context));
        }

        if !self.rules.is_empty() {
            parts.push(format!("## Rules\n{}\n", self.rules.join("\n")));
        }

        if !self.examples.is_empty() {
            parts.push(format!("## Examples\n{}\n", self.examples.join("\n\n")));
        }

        if !self.format.is_empty() {
            parts.push(format!("## Output Format\n{}\n", self.format));
        }

        parts.push(format!("## Task\n{}\n", task));

        if !self.constraints.is_empty() {
            parts.push(format!("## Constraints\n{}\n", self.constraints.join("\n")));
        }

        if !self.output_spec.is_empty() {
            parts.push(format!("## Expected Output\n{}\n", self.output_spec));
        }

        parts.join("")
    }
}

pub struct AiTask;

impl AiEndpoint {
    /// Check if a provider is disabled via AI_PROVIDERS_DISABLED env var.
    /// Example: AI_PROVIDERS_DISABLED=ollama-local,some-other
    pub fn is_disabled(label: &str) -> bool {
        let raw = std::env::var("AI_PROVIDERS_DISABLED").unwrap_or_default();
        let result = raw.split(',').any(|s| s.trim() == label);
        // Debug: print raw env var bytes for certain labels
        if label == "ollama-local" || label == "host-anthropic-proxy" {
            eprintln!(
                "[FORGE-DEBUG] is_disabled({})={} raw_env={:?} bytes={:02x?}",
                label,
                result,
                raw,
                raw.as_bytes()
            );
        }
        result
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

        // 杩藉姞椤圭洰瑙勭害锛堟潵鑷?CLAUDE.md 鎴?FORGE_PROJECT_RULES锛?
        // P1-C: Use 8-step PromptBuilder framework
        let prompt_builder = PromptBuilder::new()
            .with_role("You are a helpful AI assistant")
            .with_context(&context.task)
            .with_rule("Be concise and accurate")
            .with_rule("Follow the output format strictly")
            .with_format("Provide clear, structured output")
            .with_constraint("Do not hallucinate information")
            .with_output("Return only the requested output");

        let instruction = prompt_builder.build(base_instruction);

        // 追加项目规约（来自 CLAUDE.md 或 FORGE_PROJECT_RULES 环境变量）
        let instruction = format!("{}{}", instruction, project_rules_suffix());

        let text = context.context["text"]
            .as_str()
            .or_else(|| context.context["input"].as_str())
            .unwrap_or(&context.task)
            .to_string();

        // AI_TOON_ENABLED 鏃讹紝鑷姩灏?JSON 鏁版嵁杞?TOON 鏍煎紡鑺傜渷 token
        let text = if toon_enabled() { maybe_toon(&text) } else { text };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        // Debug: print AI_PROVIDERS_DISABLED at entry point
        eprintln!(
            "[FORGE-DEBUG] ai_task.execute() AI_PROVIDERS_DISABLED={:?}",
            std::env::var("AI_PROVIDERS_DISABLED")
        );

        let requested_model = context.context["model"].as_str();
        let endpoints: Vec<_> = AiEndpoint::enabled_candidates()
            .into_iter()
            .filter(|endpoint| requested_model.is_none_or(|model| endpoint.model == model || endpoint.label == model))
            .collect();
        // 濡傛灉鎸囧畾妯″瀷鏃犲尮閰嶇鐐癸紝涓嶉樆鏂紝鑰屾槸鍥為€€鍒板叏閮ㄥ€欓€?
        let endpoints = if endpoints.is_empty() && requested_model.is_some() {
            tracing::info!("ai_task no candidate matches requested model, falling back to all candidates");
            AiEndpoint::enabled_candidates()
        } else {
            endpoints
        };

        // Debug: log AI_PROVIDERS_DISABLED and the endpoint list
        tracing::info!(
            "ai_task AI_PROVIDERS_DISABLED={:?} endpoints={:?}",
            std::env::var("AI_PROVIDERS_DISABLED").unwrap_or_default(),
            endpoints.iter().map(|e| &e.label).collect::<Vec<_>>()
        );

        let mut last_error = String::new();

        for ep in &endpoints {
            tracing::info!("ai_task trying [{}] {} model={}", ep.label, ep.base_url, ep.model);

            match run_endpoint(&client, ep, &instruction, &text).await {
                Ok((content, usage)) => {
                    tracing::info!(
                        "ai_task [{}] success, output len={}, tokens={}",
                        ep.label,
                        content.len(),
                        usage.as_ref().map(|u| u.total_tokens).unwrap_or(0)
                    );
                    let mut output = json!({
                        "task": context.task,
                        "capability": context.capability,
                        "output": content,
                        "provider": ep.label,
                    });
                    if let Some(usage) = usage {
                        output["token_usage"] = json!({
                            "prompt_tokens": usage.prompt_tokens,
                            "completion_tokens": usage.completion_tokens,
                            "total_tokens": usage.total_tokens,
                            "cached_tokens": usage.cached_tokens,
                        });
                    }
                    return Ok(output);
                }
                Err(e) => {
                    last_error = format!("[{}] {}", ep.label, e);
                    tracing::warn!("ai_task [{}] failed: {}", ep.label, e);
                }
            }
        }

        // All providers failed 鈥?return error with details (not silent empty output)
        Ok(json!({
            "task": context.task,
            "capability": context.capability,
            "output": format!("AI service unavailable. Last error: {}", last_error),
            "error": last_error,
        }))
    }
}

/// 灏濊瘯灏?JSON 鏂囨湰杞崲涓?TOON 鏍煎紡锛屽け璐ュ垯杩斿洖鍘熸枃
fn maybe_toon(text: &str) -> String {
    // 鍙鏄庢樉鏄?JSON 鐨勬枃鏈仛杞崲锛堜互 { 鎴?[ 寮€澶达級
    let trimmed = text.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return text.to_string();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(val) => {
            let toon = format::json_to_toon(&val, 0);
            let chars_saved = trimmed.len().saturating_sub(toon.len());
            if chars_saved > 0 {
                tracing::info!(
                    "TOON: saved ~{} chars ({:.0}%)",
                    chars_saved,
                    chars_saved as f64 / trimmed.len() as f64 * 100.0
                );
            }
            // 濡傛灉鏄函 JSON 鏁版嵁锛堟寚浠ら噷濂?JSON锛夛紝鍔?TOON 鏍煎紡鎻愮ず
            format!(
                "(浠ヤ笅鏁版嵁浣跨敤 TOON 鏍煎紡锛岀被浼?YAML 浣嗘暟缁勭敤 [N]{{fields}}: 琛ㄧず)\n{}",
                toon
            )
        }
        Err(_) => text.to_string(), // 涓嶆槸鏈夋晥 JSON锛岃繑鍥炲師鏂?
    }
}

async fn run_endpoint(
    client: &reqwest::Client,
    endpoint: &AiEndpoint,
    instruction: &str,
    text: &str,
) -> Result<(String, Option<TokenUsage>)> {
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
) -> Result<(String, Option<TokenUsage>)> {
    let body = openai_request_body(endpoint, instruction, text);

    let resp = client
        .post(endpoint.chat_completions_url())
        .header("Authorization", format!("Bearer {}", endpoint.api_key))
        .json(&body)
        .send()
        .await?;
    parse_openai_response(resp).await
}

fn openai_request_body(endpoint: &AiEndpoint, instruction: &str, text: &str) -> Value {
    // 绯荤粺鎸囦护锛氬紑鍚?prompt caching 鏀寔锛圤penRouter 鍏煎锛?
    // 缂撳瓨鍚庨噸澶嶈皟鐢ㄤ粎鏀?10% 璐圭敤
    let sys_msg = json!({
        "role": "system",
        "content": instruction,
        "cache_control": {"type": "ephemeral"}
    });

    json!({
        "model": endpoint.model,
        "messages": [
            sys_msg,
            {"role": "user", "content": text}
        ],
        "max_tokens": AI_TASK_MAX_OUTPUT_TOKENS,
        "temperature": 0.3,
        "stream": false
    })
}

async fn run_anthropic_messages(
    client: &reqwest::Client,
    endpoint: &AiEndpoint,
    instruction: &str,
    text: &str,
) -> Result<(String, Option<TokenUsage>)> {
    let body = json!({
        "model": endpoint.model,
        "system": [{"type": "text", "text": instruction, "cache_control": {"type": "ephemeral"}}],
        "max_tokens": AI_TASK_MAX_OUTPUT_TOKENS,
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

async fn parse_openai_response(resp: reqwest::Response) -> Result<(String, Option<TokenUsage>)> {
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
    let (content, usage) = if parsed.is_null() {
        parse_openai_sse(&raw)
    } else {
        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .or_else(|| parsed["choices"][0]["delta"]["content"].as_str())
            .or_else(|| parsed["result"].as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        (content, parse_openai_usage(&parsed))
    };
    validate_content(&content)?;
    Ok((content, usage))
}

fn parse_openai_sse(raw: &str) -> (String, Option<TokenUsage>) {
    let mut content = String::new();
    let mut usage = None;

    for line in raw.lines() {
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(chunk) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(part) = chunk["choices"][0]["delta"]["content"]
            .as_str()
            .or_else(|| chunk["choices"][0]["message"]["content"].as_str())
        {
            content.push_str(part);
        }
        usage = parse_openai_usage(&chunk).or(usage);
    }

    (content.trim().to_string(), usage)
}

async fn parse_anthropic_response(resp: reqwest::Response) -> Result<(String, Option<TokenUsage>)> {
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
    validate_content(&content)?;
    let usage = parse_anthropic_usage(&parsed);
    Ok((content, usage))
}

fn validate_content(content: &str) -> Result<()> {
    if content.is_empty() {
        bail!("AI backend returned empty content");
    }
    if matches!(content, "鍙傛暟閿欒" | "Invalid API key.") {
        bail!("AI backend returned provider error text: {content}");
    }
    Ok(())
}

/// 浠?OpenAI 鍏煎鍝嶅簲涓彁鍙?TokenUsage
fn parse_openai_usage(parsed: &Value) -> Option<TokenUsage> {
    let usage = parsed.get("usage")?;
    let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let completion_tokens = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let total_tokens = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let cached_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens").and_then(|v| v.as_u64()))
        .or_else(|| {
            // 涓€浜?OpenAI 鍏煎绔偣鐢?cached_tokens 椤跺眰瀛楁
            usage.get("cached_tokens").and_then(|v| v.as_u64())
        })
        .unwrap_or(0);
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cached_tokens,
    })
}

/// 浠?Anthropic Messages 鍝嶅簲涓彁鍙?TokenUsage
fn parse_anthropic_usage(parsed: &Value) -> Option<TokenUsage> {
    let usage = parsed.get("usage")?;
    let prompt_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let completion_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let total_tokens = usage.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let cached_tokens = usage
        .get("input_tokens_details")
        .and_then(|d| d.get("cache_read_input_tokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cached_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::{openai_request_body, parse_openai_sse};
    use crate::config::{AiEndpoint, AiProtocol};
    use serde_json::Value;

    #[test]
    fn openai_request_budget_supports_structured_tool_followup() {
        let endpoint = AiEndpoint {
            label: "test".to_string(),
            base_url: "http://127.0.0.1:1/v1".to_string(),
            api_key: "test".to_string(),
            model: "test-model".to_string(),
            protocol: AiProtocol::OpenAiChat,
        };

        let body = openai_request_body(&endpoint, "instruction", "task");

        assert_eq!(body.get("max_tokens").and_then(Value::as_u64), Some(2048));
    }

    #[test]
    fn parses_openai_sse_content_and_usage() {
        let raw = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"AIONUI_\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"FORGE_OK\"}}],",
            "\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n",
            "data: [DONE]\n"
        );

        let (content, usage) = parse_openai_sse(raw);

        assert_eq!(content, "AIONUI_FORGE_OK");
        let usage = usage.expect("usage should be parsed");
        assert_eq!(usage.prompt_tokens, 3);
        assert_eq!(usage.completion_tokens, 2);
        assert_eq!(usage.total_tokens, 5);
    }
}
