//! forge-evolver governance middleware
//!
//! Front-door governance: clarifies ambiguous tasks, assesses risk level,
//! and decides whether to allow direct execution or require further scoping.
//! Follows Karpathy-style coding discipline.

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};
use crate::builtins::BuiltinSkill;

use super::orchestrator::call_http_ai_fallback;

const SYSTEM_CLARIFY: &str = r#"You are a governance analyst. Given a user task, determine:
1. Is the task clear and specific enough to execute? (clarity: clear|vague|ambiguous)
2. What is the risk level? (risk: low|medium|high)
3. What critical assumptions would be made if executing?
4. What is the simplest correct capability to use?

Output JSON only:
{
  "clarity": "clear|vague|ambiguous",
  "risk": "low|medium|high",
  "critical_assumptions": ["assumption 1"],
  "recommended_capability": "capability_name or 'none'",
  "rationale": "one-line explanation"
}

If clarity is not "clear", recommend gathering more information before execution. If risk is "high", require explicit user confirmation."#;

pub struct EvolverGovernance;

#[async_trait::async_trait]
impl BuiltinSkill for EvolverGovernance {
    fn name(&self) -> &'static str {
        "evolver_governance"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let task = ctx.context["task"]
            .as_str()
            .or_else(|| ctx.context["text"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();

        info!("evolver_governance: '{}'", safe_truncate(&task, 60));

        let prompt = format!("{}\n\n<task>{}</task>", SYSTEM_CLARIFY, task);
        let report = call_http_ai_fallback(&prompt, "evolver_governance").await;

        match report.output {
            Some(output) => {
                let parsed = serde_json::from_str(&output).unwrap_or_else(|_| {
                    json!({"clarity": "unknown", "risk": "unknown", "raw_output": output})
                });
                Ok(json!({
                    "governance": parsed,
                    "adapter": {
                        "instruction": "Review governance assessment before proceeding. If clarity != 'clear' or risk == 'high', gather more info first.",
                        "task": task
                    }
                }))
            }
            None => Ok(json!({
                "governance": {
                    "clarity": "unknown",
                    "risk": "medium",
                    "rationale": "AI governance unavailable, defaulting to permissive"
                },
                "adapter": {"instruction": "AI governance unavailable. Proceed with caution.", "task": task}
            })),
        }
    }
}

fn safe_truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}
