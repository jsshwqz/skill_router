//! AI task builtin skill: ai_task
//! Supports fallback chain: primary AI provider -> Ollama local -> error with details

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

pub struct AiTask;

/// An AI provider endpoint to try.
struct AiEndpoint {
    label: &'static str,
    base_url: String,
    api_key: String,
    model: String,
}

impl AiEndpoint {
    /// Check if a provider is disabled via AI_PROVIDERS_DISABLED env var.
    /// Example: AI_PROVIDERS_DISABLED=ollama-local,some-other
    fn is_disabled(label: &str) -> bool {
        std::env::var("AI_PROVIDERS_DISABLED")
            .map(|v| v.split(',').any(|s| s.trim() == label))
            .unwrap_or(false)
    }

    fn from_env() -> Vec<Self> {
        let mut endpoints = Vec::new();

        // Primary: user-configured AI service
        let primary_url = std::env::var("AI_BASE_URL").unwrap_or_default();
        let primary_key = std::env::var("AI_API_KEY").unwrap_or_default();
        let primary_model = std::env::var("AI_MODEL").unwrap_or_default();

        if !primary_url.is_empty() && !primary_key.is_empty() && !primary_model.is_empty()
            && !Self::is_disabled("primary")
        {
            endpoints.push(AiEndpoint {
                label: "primary",
                base_url: primary_url,
                api_key: primary_key,
                model: primary_model,
            });
        }

        // Fallback: local Ollama (only if not disabled)
        if !Self::is_disabled("ollama-local") {
            endpoints.push(AiEndpoint {
                label: "ollama-local",
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: "ollama".to_string(),
                model: "qwen2.5:7b".to_string(),
            });
        }

        endpoints
    }
}

#[async_trait::async_trait]
impl BuiltinSkill for AiTask {
    fn name(&self) -> &'static str {
        "ai_task"
    }

    async fn execute(
        &self,
        skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let instruction = skill.metadata.instruction.as_deref().ok_or_else(|| {
            anyhow!(
                "skill '{}' uses builtin:ai_task but has no 'instruction' field in skill.json",
                skill.metadata.name
            )
        })?;

        let text = context.context["text"]
            .as_str()
            .or_else(|| context.context["input"].as_str())
            .unwrap_or(&context.task)
            .to_string();

        let body = |model: &str| {
            json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": instruction},
                    {"role": "user", "content": text}
                ],
                "temperature": 0.3
            })
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        let endpoints = AiEndpoint::from_env();
        let mut last_error = String::new();

        for ep in &endpoints {
            tracing::info!("ai_task trying [{}] {} model={}", ep.label, ep.base_url, ep.model);

            let result = client
                .post(format!("{}/chat/completions", ep.base_url))
                .header("Authorization", format!("Bearer {}", ep.api_key))
                .json(&body(&ep.model))
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    let raw = resp.text().await.unwrap_or_default();

                    if status.is_success() {
                        let parsed: Value = serde_json::from_str(&raw).unwrap_or_default();
                        let content = parsed["choices"][0]["message"]["content"]
                            .as_str()
                            .or_else(|| parsed["result"].as_str())
                            .unwrap_or("")
                            .to_string();

                        if !content.is_empty() {
                            tracing::info!("ai_task [{}] success, output len={}", ep.label, content.len());
                            return Ok(json!({
                                "task": context.task,
                                "capability": context.capability,
                                "output": content,
                                "provider": ep.label,
                            }));
                        }
                        last_error = format!("[{}] returned empty content", ep.label);
                    } else {
                        // Extract error message from response
                        let err_msg = serde_json::from_str::<Value>(&raw)
                            .ok()
                            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
                            .unwrap_or_else(|| format!("HTTP {}", status));
                        last_error = format!("[{}] {}", ep.label, err_msg);
                        tracing::warn!("ai_task [{}] failed: {}", ep.label, last_error);
                    }
                }
                Err(e) => {
                    last_error = format!("[{}] {}", ep.label, e);
                    tracing::warn!("ai_task [{}] connection error: {}", ep.label, e);
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
