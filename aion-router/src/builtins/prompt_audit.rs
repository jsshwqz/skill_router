//! Prompt quality auditor — 8-step framework compliance check
//!
//! Evaluates a prompt against the CLAUDE.md 8-step framework:
//! Role, Task Context, Rules, Examples, XML Input, Output Format, CoT, Anti-hallucination
//!
//! Returns compliance score and actionable improvement suggestions.

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};
use crate::builtins::BuiltinSkill;

use super::orchestrator::call_http_ai_fallback;

const AUDIT_SYSTEM: &str = r#"You are a prompt engineering auditor. Evaluate the given prompt against this 8-step framework:

1. Role assignment — starts with "You are a..." or equivalent identity
2. Task context — explains what to do and why
3. Detailed rules — boundaries, constraints, do/don't
4. Examples (few-shot) — provides 1-2 examples wrapped in XML tags
5. Data in XML tags — variable input wrapped in <tag>...</tag>
6. Output format — specific format instruction near bottom
7. Chain of Thought — "first analyze step by step" for complex tasks
8. Anti-hallucination — gives an out: "if unsure, say unknown"

Output JSON only:
{
  "score": 0.0,
  "framework": {
    "role_assignment": {"present": true, "suggestion": ""},
    "task_context": {"present": true, "suggestion": ""},
    "rules": {"present": true, "suggestion": ""},
    "examples": {"present": false, "suggestion": "Add 1-2 few-shot examples wrapped in <example> tags"},
    "xml_input_tags": {"present": true, "suggestion": ""},
    "output_format": {"present": true, "suggestion": ""},
    "chain_of_thought": {"present": false, "suggestion": "Add 'first analyze step by step' for complex reasoning"},
    "anti_hallucination": {"present": false, "suggestion": "Add an out: 'if unsure, say unknown'"}
  },
  "missing_items": ["examples", "anti_hallucination"],
  "critical_issues": [],
  "improved_prompt": "",
  "summary": "3 of 8 framework items missing. Score: 5/8"
}

Be strict: if an element is partially present but weak, mark it as false and explain why."#;

pub struct PromptAudit;

#[async_trait::async_trait]
impl BuiltinSkill for PromptAudit {
    fn name(&self) -> &'static str {
        "prompt_audit"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let prompt = ctx.context["prompt"]
            .as_str()
            .unwrap_or(&ctx.task)
            .to_string();
        let model = ctx.context["model"].as_str().unwrap_or("claude");

        info!("prompt_audit: {} chars, model={}", prompt.len(), model);

        if prompt.trim().is_empty() {
            return Ok(json!({
                "score": 0.0,
                "summary": "empty prompt — nothing to audit",
                "framework": {},
                "missing_items": ["prompt"],
                "critical_issues": ["No prompt provided"]
            }));
        }

        let full_prompt = format!(
            "{}Target model: {}\n\n<prompt_to_audit>\n{}\n</prompt_to_audit>",
            AUDIT_SYSTEM, model, prompt
        );

        let report = call_http_ai_fallback(&full_prompt, "prompt_audit").await;

        match report.output {
            Some(output) => {
                let audit = serde_json::from_str(&output).unwrap_or_else(|_| {
                    json!({
                        "score": 0.0,
                        "summary": "audit parse failed — raw response below",
                        "raw_output": output
                    })
                });
                Ok(json!({
                    "audit": audit,
                    "target_model": model,
                    "prompt_chars": prompt.len(),
                    "adaptation_hint": match model {
                        "gemini" => "Gemini requires Persona/Task/Context/Format four elements. Convert XML tags to this structure.",
                        "gpt" | "openai" => "GPT-5.5 prefers outcome-first over step-by-step. Consider shortening.",
                        "deepseek" => "DeepSeek uses CO-STAR framework. Add Context/Objective/Style/Tone/Audience/Response.",
                        _ => "Claude-native XML format should work as-is.",
                    }
                }))
            }
            None => Ok(json!({
                "audit": {"score": 0.0, "summary": "AI auditor unavailable — manual review needed",
                          "framework": {}, "missing_items": [], "critical_issues": []},
                "target_model": model,
                "prompt_chars": prompt.len(),
            })),
        }
    }
}
