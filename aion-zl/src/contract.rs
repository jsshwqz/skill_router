//! Task Contract — 将自然语言任务编译为结构化契约
//! 三个 Sensor 的基础：定义"什么叫完成"
//!
//! 灵感来源：AionForge 自审报告的 P0 建议

use crate::{ai, engine::Engine};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

const CONTRACT_SYSTEM: &str = r#"You are a task contract compiler. Convert natural language tasks into structured contracts.

A contract defines:
1. What "done" looks like (acceptance_criteria)
2. Required outputs (expected_outputs)
3. Context needed before execution (required_context)
4. How to verify completion (verification_method)

Output JSON:
{
  "task_summary": "one-line restatement",
  "acceptance_criteria": ["criterion 1", "criterion 2"],
  "expected_outputs": [
    { "type": "code|text|data|config|action", "description": "what this output is" }
  ],
  "required_context": ["what info is needed before starting"],
  "verification_method": "how to check if done correctly",
  "complexity": "low|medium|high",
  "estimated_steps": 3
}"#;

const SUFFICIENCY_SYSTEM: &str = r#"You are a context sufficiency sensor.
Given a task contract and available context, determine if there is enough information to proceed.

Output JSON:
{
  "sufficient": true|false,
  "confidence": 0.0-1.0,
  "missing": ["what info is missing"],
  "recommendation": "proceed|gather_more|clarify_with_user"
}"#;

const VERIFY_SYSTEM: &str = r#"You are a result verification sensor.
Given a task contract and execution result, verify if the result meets the acceptance criteria.

Output JSON:
{
  "passed": true|false,
  "score": 0.0-1.0,
  "criteria_results": [
    { "criterion": "...", "met": true|false, "evidence": "..." }
  ],
  "verdict": "accept|retry|escalate",
  "feedback": "what to fix if retry"
}"#;

const DRIFT_SYSTEM: &str = r#"You are an execution drift sensor.
Given the original task contract and current execution state, detect if the work is drifting off target.

Output JSON:
{
  "on_track": true|false,
  "drift_score": 0.0-1.0,
  "drift_description": "what went off track",
  "correction": "how to get back on track"
}"#;

// ── Data structures ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedOutput {
    #[serde(rename = "type")]
    pub output_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContract {
    pub task_summary: String,
    pub acceptance_criteria: Vec<String>,
    pub expected_outputs: Vec<ExpectedOutput>,
    pub required_context: Vec<String>,
    pub verification_method: String,
    pub complexity: String,
    pub estimated_steps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SufficiencyResult {
    pub sufficient: bool,
    pub confidence: f32,
    pub missing: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion: String,
    pub met: bool,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub passed: bool,
    pub score: f32,
    pub criteria_results: Vec<CriterionResult>,
    pub verdict: String,
    pub feedback: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResult {
    pub on_track: bool,
    pub drift_score: f32,
    pub drift_description: String,
    pub correction: String,
}

// ── Engine methods ──

impl Engine {
    /// Sensor P0: 将任务编译为结构化契约
    pub async fn compile_contract(&self, task: &str) -> Result<TaskContract> {
        info!("Compiling task contract...");
        let raw = ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            CONTRACT_SYSTEM, task,
        ).await?;

        Ok(TaskContract {
            task_summary: raw["task_summary"].as_str().unwrap_or(task).into(),
            acceptance_criteria: raw["acceptance_criteria"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            expected_outputs: raw["expected_outputs"].as_array()
                .map(|a| a.iter().map(|v| ExpectedOutput {
                    output_type: v["type"].as_str().unwrap_or("text").into(),
                    description: v["description"].as_str().unwrap_or("").into(),
                }).collect())
                .unwrap_or_default(),
            required_context: raw["required_context"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            verification_method: raw["verification_method"].as_str().unwrap_or("").into(),
            complexity: raw["complexity"].as_str().unwrap_or("medium").into(),
            estimated_steps: raw["estimated_steps"].as_u64().unwrap_or(3) as u32,
        })
    }

    /// Sensor P0: 上下文充分性检测 — 执行前判断"是否理解够了"
    pub async fn check_sufficiency(&self, contract: &TaskContract, context: &str) -> Result<SufficiencyResult> {
        info!("Checking context sufficiency...");
        let prompt = format!(
            "Contract:\n{}\n\nAvailable context:\n{}",
            serde_json::to_string_pretty(contract)?,
            context,
        );
        let raw = ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            SUFFICIENCY_SYSTEM, &prompt,
        ).await?;

        Ok(SufficiencyResult {
            sufficient: raw["sufficient"].as_bool().unwrap_or(false),
            confidence: raw["confidence"].as_f64().unwrap_or(0.5) as f32,
            missing: raw["missing"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            recommendation: raw["recommendation"].as_str().unwrap_or("gather_more").into(),
        })
    }

    /// Sensor P1: 结果契约验证 — 执行后对照契约校验输出
    pub async fn verify_result(&self, contract: &TaskContract, result: &str) -> Result<VerifyResult> {
        info!("Verifying result against contract...");
        let prompt = format!(
            "Contract:\n{}\n\nExecution result:\n{}",
            serde_json::to_string_pretty(contract)?,
            result,
        );
        let raw = ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            VERIFY_SYSTEM, &prompt,
        ).await?;

        Ok(VerifyResult {
            passed: raw["passed"].as_bool().unwrap_or(false),
            score: raw["score"].as_f64().unwrap_or(0.0) as f32,
            criteria_results: raw["criteria_results"].as_array()
                .map(|a| a.iter().map(|v| CriterionResult {
                    criterion: v["criterion"].as_str().unwrap_or("").into(),
                    met: v["met"].as_bool().unwrap_or(false),
                    evidence: v["evidence"].as_str().unwrap_or("").into(),
                }).collect())
                .unwrap_or_default(),
            verdict: raw["verdict"].as_str().unwrap_or("retry").into(),
            feedback: raw["feedback"].as_str().unwrap_or("").into(),
        })
    }

    /// Sensor P1: 执行偏航检测 — 监控过程是否偏离目标
    pub async fn detect_drift(&self, contract: &TaskContract, current_state: &str) -> Result<DriftResult> {
        info!("Detecting execution drift...");
        let prompt = format!(
            "Original contract:\n{}\n\nCurrent execution state:\n{}",
            serde_json::to_string_pretty(contract)?,
            current_state,
        );
        let raw = ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            DRIFT_SYSTEM, &prompt,
        ).await?;

        Ok(DriftResult {
            on_track: raw["on_track"].as_bool().unwrap_or(true),
            drift_score: raw["drift_score"].as_f64().unwrap_or(0.0) as f32,
            drift_description: raw["drift_description"].as_str().unwrap_or("").into(),
            correction: raw["correction"].as_str().unwrap_or("").into(),
        })
    }
}
