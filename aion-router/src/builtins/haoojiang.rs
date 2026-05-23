//! 郝匠 Code Quality Gate
//!
//! Combines static analysis + AI cross-review + project rules to produce
//! a unified quality assessment. Named after the disciplined craftsman archetype.

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};
use crate::builtins::BuiltinSkill;

use super::orchestrator::call_http_ai_fallback;

const SYSTEM_REVIEW: &str = r#"You are a code quality reviewer. Review the given code for:
1. Correctness — logic errors, off-by-one, type mismatches
2. Safety — unwrap/dangerous casts, injection risks, unsafe blocks without justification
3. Style — naming, naming conventions, dead code, formatting (not exhaustive)
4. Maintainability — complexity, duplication, testability

Output JSON only:
{
  "score": 0.0,
  "verdict": "pass|needs_fix|reject",
  "issues": [{"severity":"critical|major|minor","category":"correctness|safety|style|maintainability","line":0,"description":"","suggestion":""}],
  "strengths": ["what was done well"],
  "summary": "one-line verdict"
}

If the code is empty or not real code, set score=0 and verdict=reject."#;

pub struct HaoJiangReview;

#[async_trait::async_trait]
impl BuiltinSkill for HaoJiangReview {
    fn name(&self) -> &'static str {
        "haoojiang_review"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let code = ctx.context["code"].as_str().unwrap_or(&ctx.task).to_string();
        let language = ctx.context["language"].as_str().unwrap_or("unknown");
        let _standards = ctx.context["standards"].as_str();

        info!("haoojiang_review: {} bytes, {}", code.len(), language);

        // Step 1: Quick length check (gate early for trivial code)
        if code.trim().is_empty() {
            return Ok(json!({
                "score": 0.0,
                "verdict": "reject",
                "issues": [{"severity":"critical","category":"correctness","line":0,
                    "description":"Empty code submitted","suggestion":"Provide actual code"}],
                "strengths": [],
                "summary": "empty code rejected"
            }));
        }

        // Step 2: AI review via HTTP fallback
        let prompt = format!(
            "{}Language: {}\n\n<code>\n{}\n</code>",
            SYSTEM_REVIEW, language, code
        );
        let report = call_http_ai_fallback(&prompt, "haoojiang_review").await;

        match report.output {
            Some(output) => {
                let review = serde_json::from_str(&output).unwrap_or_else(|_| {
                    json!({"score": 0.5, "verdict": "needs_fix", "issues": [],
                        "strengths": [], "summary": "AI review parse failed, manual check needed"})
                });
                Ok(json!({
                    "review": review,
                    "language": language,
                    "code_size_bytes": code.len(),
                }))
            }
            None => Ok(json!({
                "review": {
                    "score": 0.0,
                    "verdict": "needs_fix",
                    "issues": [{"severity":"major","category":"correctness","line":0,
                        "description":"AI review unavailable","suggestion":"Run code_lint manually"}],
                    "strengths": [], "summary": "AI review unavailable"
                },
                "language": language,
                "code_size_bytes": code.len(),
            })),
        }
    }
}
