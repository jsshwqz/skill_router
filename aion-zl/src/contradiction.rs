//! 矛盾论 — Contradiction analysis
//! "集中优势兵力，各个歼灭敌人"

use crate::{ai, engine::Engine};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

const SYSTEM: &str = r#"You are an expert task analyst using contradiction analysis.
Given a complex task, decompose it and identify contradictions (bottlenecks, tensions).

For each contradiction:
- description, is_principal (only ONE true), affected_step, severity (1-10), resolution

Also provide:
- principal_contradiction: one-line summary of the main blocker
- recommended_focus: which subtask to concentrate on
- resource_allocation: { "step_name": weight_0_to_1 } (sum to 1.0)

Output JSON:
{
  "contradictions": [...],
  "principal_contradiction": "...",
  "recommended_focus": "...",
  "resource_allocation": { "step1": 0.5, "step2": 0.3, "step3": 0.2 }
}"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub description: String,
    pub is_principal: bool,
    pub affected_step: Option<String>,
    pub severity: u8,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionReport {
    pub task: String,
    pub contradictions: Vec<Contradiction>,
    pub principal_contradiction: Option<String>,
    pub recommended_focus: String,
    pub resource_allocation: HashMap<String, f32>,
}

impl Engine {
    pub async fn contradiction_analyze(&self, task: &str) -> Result<ContradictionReport> {
        info!("Analyzing contradictions...");
        let raw = ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            SYSTEM, &format!("Task to analyze:\n{}", task),
        ).await?;

        let contradictions: Vec<Contradiction> = raw["contradictions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
            .unwrap_or_default();

        let resource_allocation: HashMap<String, f32> = raw["resource_allocation"]
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0) as f32)).collect())
            .unwrap_or_default();

        info!("Found {} contradictions", contradictions.len());

        Ok(ContradictionReport {
            task: task.into(),
            contradictions,
            principal_contradiction: raw["principal_contradiction"].as_str().map(String::from),
            recommended_focus: raw["recommended_focus"].as_str().unwrap_or("unknown").into(),
            resource_allocation,
        })
    }
}
