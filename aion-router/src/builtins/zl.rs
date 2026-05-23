//! aion-zl dialectical tools as Forge builtins
//!
//! 8 builtin skills: strategic_plan, task_dialectic, contradiction_analyze,
//! compile_contract, check_sufficiency, verify_result, detect_drift, dialectical_retry.
//!
//! Each extracts input from ExecutionContext, builds a prompt, calls AI via HTTP fallback,
//! and returns structured JSON. Prompts follow the 8-step framework from CLAUDE.md.

use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use aion_types::types::{ExecutionContext, SkillDefinition};
use crate::builtins::BuiltinSkill;

use super::orchestrator::call_http_ai_fallback;

// ── System prompts (8-step framework: role + task + rules + output + anti-hallucination) ──

const SYSTEM_STRATEGIC_PLAN: &str = r#"You are a strategic planner using the "protracted war" framework.
Assess information availability: scarce → defense, moderate → stalemate, clear → offense.
Output JSON only:
{
  "current_phase": "defense|stalemate|offense",
  "phase_rationale": "why this phase",
  "estimated_complexity": "low|medium|high",
  "steps": [
    {"name":"step1","phase":"defense","action":"","capability":"","resource_weight":0.0}
  ]
}
If uncertain about the phase, default to "defense"."#;

const SYSTEM_DIALECTIC: &str = r#"You run a three-step dialectical process: thesis → antithesis → synthesis.
Given a task, produce a solution (thesis), then critique it (antithesis), then combine the best of both (synthesis).
Output JSON only:
{
  "thesis": {"content":"","strengths":[],"weaknesses":[],"confidence":0.0},
  "antithesis": {"content":"","strengths":[],"weaknesses":[],"confidence":0.0},
  "synthesis": {"content":"","strengths":[],"weaknesses":[],"confidence":0.0}
}
If the task is unclear, set all confidence to 0. Do not fabricate a solution when ambiguous."#;

const SYSTEM_CONTRADICTION: &str = r#"You are a contradiction analyst. Given a complex task, decompose it and identify bottlenecks and tensions.
Find the principal contradiction (main blocker). Output JSON only:
{
  "contradictions":[{"description":"","is_principal":false,"affected_step":"","severity":1,"resolution":""}],
  "principal_contradiction":"",
  "recommended_focus":"",
  "resource_allocation":{"step1":0.5}
}
If no contradictions found, output empty array and principal_contradiction: "none"."#;

const SYSTEM_COMPILE_CONTRACT: &str = r#"You are a task contract compiler. Convert natural language tasks into structured contracts.
Output JSON only:
{
  "task_summary": "one-line restatement",
  "acceptance_criteria": ["criterion 1"],
  "expected_outputs": [{"type":"code|text|data","description":""}],
  "required_context": ["what info needed"],
  "verification_method": "how to verify",
  "complexity": "low|medium|high",
  "estimated_steps": 1
}
If unsure about a field, use "unknown" for strings, [] for arrays."#;

const SYSTEM_SUFFICIENCY: &str = r#"You are a context sufficiency sensor. Given a contract and available context, determine if enough information exists.
Output JSON only:
{
  "sufficient": true,
  "confidence": 0.0,
  "missing": [],
  "recommendation": "proceed|gather_more|clarify_with_user"
}
If uncertain about confidence, set 0.5."#;

const SYSTEM_VERIFY: &str = r#"You are a result verification sensor. Check if execution results meet the contract's acceptance criteria.
Output JSON only:
{
  "passed": true,
  "score": 0.0,
  "criteria_results": [{"criterion":"","met":false,"evidence":""}],
  "verdict": "accept|retry|escalate",
  "feedback": ""
}
If no evidence found for a criterion, mark not met. If result is empty, score=0."#;

const SYSTEM_DRIFT: &str = r#"You are an execution drift sensor. Compare current state against original contract to detect off-target work.
Output JSON only:
{
  "on_track": true,
  "drift_score": 0.0,
  "drift_description": "",
  "correction": ""
}
If unsure about drift, set score < 0.3."#;

const SYSTEM_RETRY: &str = r#"You are a root cause analyst. Given a task, strategy used, and error, analyze the failure.
Output JSON only:
{
  "root_cause": "",
  "lesson": "",
  "next_strategy": "concrete alternative approach"
}
If you cannot determine root cause with confidence, set root_cause to "unknown"."#;

// ── Helper ──

fn require_text(ctx: &ExecutionContext) -> String {
    ctx.context["task"]
        .as_str()
        .or_else(|| ctx.context["text"].as_str())
        .or_else(|| ctx.context["input"].as_str())
        .unwrap_or(&ctx.task)
        .to_string()
}

async fn call_ai(prompt: &str) -> Value {
    let report = call_http_ai_fallback(prompt, "zl").await;
    match report.output {
        Some(output) => {
            serde_json::from_str(&output).unwrap_or_else(|_| {
                json!({"raw_output": output, "parse_error": "response was not valid JSON"})
            })
        }
        None => json!({"error": report.error_message.unwrap_or_else(|| "AI call failed".into())}),
    }
}

// ── Builtin Skills ──

macro_rules! define_zl_skill {
    ($name:ident, $display:expr, $system:expr) => {
        pub struct $name;
        #[async_trait::async_trait]
        impl BuiltinSkill for $name {
            fn name(&self) -> &'static str { $display }
            async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
                let task = require_text(ctx);
                info!("{}: '{}'", $display, safe_truncate(&task, 50));
                let prompt = format!("{}\n\n<task>{}</task>", $system, task);
                Ok(call_ai(&prompt).await)
            }
        }
    };
}

fn safe_truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}

// Define all 8 skills using the macro
define_zl_skill!(ZLStrategicPlan, "strategic_plan", SYSTEM_STRATEGIC_PLAN);
define_zl_skill!(ZLTaskDialectic, "task_dialectic", SYSTEM_DIALECTIC);
define_zl_skill!(ZLContradictionAnalyze, "contradiction_analyze", SYSTEM_CONTRADICTION);
define_zl_skill!(ZLCompileContract, "compile_contract", SYSTEM_COMPILE_CONTRACT);
define_zl_skill!(ZLCheckSufficiency, "check_sufficiency", SYSTEM_SUFFICIENCY);
define_zl_skill!(ZLVerifyResult, "verify_result", SYSTEM_VERIFY);
define_zl_skill!(ZLDetectDrift, "detect_drift", SYSTEM_DRIFT);
define_zl_skill!(ZLDialecticalRetry, "dialectical_retry", SYSTEM_RETRY);
