//! 图像分析 builtin：image_describe
//!
//! 接受图片 URL，通过 AI 视觉能力进行分析描述。
//! URL 直接传给 vision API（无需 base64 编码）。

use anyhow::Result;
use serde_json::{json, Value};

use super::BuiltinSkill;
use crate::config::candidate_ai_endpoints;
use aion_types::types::{ExecutionContext, SkillDefinition};

pub struct ImageDescribe;

#[async_trait::async_trait]
impl BuiltinSkill for ImageDescribe {
    fn name(&self) -> &'static str {
        "image_describe"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let url = ctx.context["url"]
            .as_str()
            .or_else(|| ctx.context["text"].as_str())
            .or_else(|| ctx.context["input"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();

        if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("file://") {
            return Ok(json!({
                "error": "image_describe requires a URL (http/https/file). 本地文件暂时不支持，请先上传或使用 URL。",
                "input": url,
            }));
        }

        let instruction = ctx.context["instruction"]
            .as_str()
            .unwrap_or("请详细描述这张图片的内容，包括：主体对象、场景环境、文字内容（如有）、颜色和构图。");

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        let endpoints = candidate_ai_endpoints();
        // 只选 OpenAI 兼容的 endpoint（可能有 Anthropic 等其他协议的端点）
        let vision_endpoints: Vec<_> = endpoints
            .iter()
            .filter(|ep| ep.protocol == crate::config::AiProtocol::OpenAiChat)
            .collect();

        if vision_endpoints.is_empty() {
            return Ok(json!({
                "error": "没有可用的 OpenAI 兼容 AI 端点（vision 需要 OpenAiChat 协议）",
                "note": "请配置 AI_BASE_URL / AI_API_KEY / AI_MODEL",
            }));
        }

        for ep in &vision_endpoints {
            let body = json!({
                "model": ep.model,
                "messages": [
                    {"role": "system", "content": "你是一个图像分析助手。", "cache_control": {"type": "ephemeral"}},
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": instruction},
                            {"type": "image_url", "image_url": { "url": &url }}
                        ]
                    }
                ],
                "max_tokens": 2048,
            });

            let resp = match client
                .post(ep.chat_completions_url())
                .header("Authorization", format!("Bearer {}", ep.api_key))
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("image_describe [{}] request failed: {}", ep.label, e);
                    continue;
                }
            };

            let status = resp.status();
            let raw = resp.text().await.unwrap_or_default();
            let parsed: Value = serde_json::from_str(&raw).unwrap_or_default();

            if !status.is_success() {
                tracing::warn!("image_describe [{}] HTTP {}: {}", ep.label, status, raw);
                continue;
            }

            let content = parsed["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_string();

            if content.is_empty() {
                continue;
            }

            return Ok(json!({
                "url": url,
                "description": content,
                "provider": ep.label,
            }));
        }

        Ok(json!({
            "error": "所有 AI 端点均失败",
            "url": url,
        }))
    }
}
