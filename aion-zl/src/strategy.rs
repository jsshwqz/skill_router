//! 论持久战 — Three-phase strategic planning
//! Defense → Stalemate → Offense

use crate::{ai, engine::Engine};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

const SYSTEM: &str = r#"You are a strategic planner using the "protracted war" framework.

Three phases:
1. DEFENSE: Info scarce. Lightweight probing (echo, discovery_search, web_search, memory_recall).
2. STALEMATE: Iterate small wins (code_generate, code_lint, text_summarize, json_parse).
3. OFFENSE: Concentrate force (task_pipeline, agent_gather, code_test, parallel execution).

Each step specifies an aion-forge capability to call.

Output JSON:
{
  "current_phase": "defense"|"stalemate"|"offense",
  "phase_rationale": "why",
  "estimated_complexity": "low"|"medium"|"high",
  "steps": [
    { "name": "...", "phase": "...", "action": "...", "capability": "...", "resource_weight": 0.0-1.0 }
  ]
}"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicStep {
    pub name: String,
    pub phase: String,
    pub action: String,
    pub capability: String,
    pub resource_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategicPlan {
    pub task: String,
    pub current_phase: String,
    pub phase_rationale: String,
    pub steps: Vec<StrategicStep>,
    pub estimated_complexity: String,
}

impl Engine {
    pub async fn strategic_plan(&self, task: &str) -> Result<StrategicPlan> {
        info!("Creating strategic plan...");

        let memories = self.recall(task).unwrap_or_default();
        let mem_ctx = if memories.is_empty() {
            String::new()
        } else {
            let summaries: Vec<_> = memories.iter().map(|m| m.content.as_str()).collect();
            format!("\n\nPrior experience:\n{}", summaries.join("\n"))
        };

        let raw = ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            SYSTEM, &format!("Task:\n{}{}", task, mem_ctx),
        ).await?;

        let steps: Vec<StrategicStep> = raw["steps"]
            .as_array()
            .map(|arr| arr.iter().map(|v| StrategicStep {
                name: v["name"].as_str().unwrap_or("").into(),
                phase: v["phase"].as_str().unwrap_or("defense").into(),
                action: v["action"].as_str().unwrap_or("").into(),
                capability: v["capability"].as_str().unwrap_or("echo").into(),
                resource_weight: v["resource_weight"].as_f64().unwrap_or(0.2) as f32,
            }).collect())
            .unwrap_or_default();

        info!("Plan: phase={}, steps={}", raw["current_phase"].as_str().unwrap_or("?"), steps.len());

        Ok(StrategicPlan {
            task: task.into(),
            current_phase: raw["current_phase"].as_str().unwrap_or("defense").into(),
            phase_rationale: raw["phase_rationale"].as_str().unwrap_or("").into(),
            steps,
            estimated_complexity: raw["estimated_complexity"].as_str().unwrap_or("medium").into(),
        })
    }
}
