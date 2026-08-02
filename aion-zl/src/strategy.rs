//! 论持久战 — Three-phase strategic planning
//! Defense → Stalemate → Offense

use crate::{ai, engine::Engine};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

const SYSTEM: &str = r#"You are a strategic planner using the "protracted war" framework.

First, assess how much information is available about the task. If information is scarce, you are likely in defense. If you have moderate information and can iterate, stalemate. Only if the path is clear and you have sufficient resources should you choose offense.

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
}

If you are uncertain about the current phase, default to "defense"."#;

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

        let raw = ai::chat_json_deterministic(
            &self.http,
            &self.ai_base_url,
            &self.ai_api_key,
            &self.ai_model,
            SYSTEM,
            &format!("Task:\n{}{}", task, mem_ctx),
        )
        .await?;

        let plan = plan_from_json(&raw, task);

        info!("Plan: phase={}, steps={}", plan.current_phase, plan.steps.len());

        Ok(plan)
    }
}

/// Map a raw AI strategy JSON response onto a [`StrategicPlan`].
fn plan_from_json(raw: &serde_json::Value, task: &str) -> StrategicPlan {
    let steps: Vec<StrategicStep> = raw["steps"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|v| StrategicStep {
                    name: v["name"].as_str().unwrap_or("").into(),
                    phase: v["phase"].as_str().unwrap_or("defense").into(),
                    action: v["action"].as_str().unwrap_or("").into(),
                    capability: v["capability"].as_str().unwrap_or("echo").into(),
                    resource_weight: v["resource_weight"].as_f64().unwrap_or(0.2) as f32,
                })
                .collect()
        })
        .unwrap_or_default();

    StrategicPlan {
        task: task.into(),
        current_phase: raw["current_phase"].as_str().unwrap_or("defense").into(),
        phase_rationale: raw["phase_rationale"].as_str().unwrap_or("").into(),
        steps,
        estimated_complexity: raw["estimated_complexity"].as_str().unwrap_or("medium").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The framework's three-phase capability sets (lightweight probing → iteration → concentration).
    const DEFENSE_CAPS: &[&str] = &["echo", "discovery_search", "web_search", "memory_recall"];
    const STALEMATE_CAPS: &[&str] = &["code_generate", "code_lint", "text_summarize", "json_parse"];
    const OFFENSE_CAPS: &[&str] = &["task_pipeline", "agent_gather", "code_test"];

    fn plan_raw(phase: &str, steps: Vec<serde_json::Value>) -> serde_json::Value {
        json!({
            "current_phase": phase,
            "phase_rationale": "because",
            "estimated_complexity": "high",
            "steps": steps
        })
    }

    fn step(name: &str, phase: &str, capability: &str, weight: f64) -> serde_json::Value {
        json!({
            "name": name,
            "phase": phase,
            "action": format!("do {}", name),
            "capability": capability,
            "resource_weight": weight
        })
    }

    #[test]
    fn defense_phase_maps_to_lightweight_probe_capabilities() {
        let caps = DEFENSE_CAPS
            .iter()
            .map(|c| step(&format!("probe-{c}"), "defense", c, 0.2))
            .collect();
        let plan = plan_from_json(&plan_raw("defense", caps), "task");
        assert_eq!(plan.current_phase, "defense");
        assert_eq!(plan.steps.len(), DEFENSE_CAPS.len());
        for (s, cap) in plan.steps.iter().zip(DEFENSE_CAPS.iter()) {
            assert_eq!(s.phase, "defense");
            assert_eq!(&s.capability, cap);
        }
    }

    #[test]
    fn stalemate_phase_maps_to_iteration_capabilities() {
        let caps = STALEMATE_CAPS
            .iter()
            .map(|c| step(&format!("iterate-{c}"), "stalemate", c, 0.5))
            .collect();
        let plan = plan_from_json(&plan_raw("stalemate", caps), "task");
        assert_eq!(plan.current_phase, "stalemate");
        assert_eq!(plan.steps.len(), STALEMATE_CAPS.len());
        for (s, cap) in plan.steps.iter().zip(STALEMATE_CAPS.iter()) {
            assert_eq!(s.phase, "stalemate");
            assert_eq!(&s.capability, cap);
        }
    }

    #[test]
    fn offense_phase_maps_to_concentration_capabilities() {
        let caps = OFFENSE_CAPS
            .iter()
            .map(|c| step(&format!("attack-{c}"), "offense", c, 0.9))
            .collect();
        let plan = plan_from_json(&plan_raw("offense", caps), "task");
        assert_eq!(plan.current_phase, "offense");
        assert_eq!(plan.steps.len(), OFFENSE_CAPS.len());
        for (s, cap) in plan.steps.iter().zip(OFFENSE_CAPS.iter()) {
            assert_eq!(s.phase, "offense");
            assert_eq!(&s.capability, cap);
        }
    }

    #[test]
    fn plan_from_json_preserves_step_weight_and_action() {
        let raw = plan_raw("offense", vec![step("execute", "offense", "code_test", 0.75)]);
        let plan = plan_from_json(&raw, "t");
        assert_eq!(plan.steps[0].name, "execute");
        assert_eq!(plan.steps[0].action, "do execute");
        assert_eq!(plan.steps[0].resource_weight, 0.75);
        assert_eq!(plan.phase_rationale, "because");
        assert_eq!(plan.estimated_complexity, "high");
    }

    #[test]
    fn plan_from_json_defaults_to_defense_for_missing_phase() {
        let plan = plan_from_json(&json!({}), "t");
        assert_eq!(plan.current_phase, "defense", "uncertain defaults to defense");
        assert_eq!(plan.estimated_complexity, "medium");
        assert!(plan.steps.is_empty());
        assert_eq!(plan.task, "t");
    }

    #[test]
    fn strategic_plan_serde_roundtrip() {
        let p = StrategicPlan {
            task: "t".into(),
            current_phase: "offense".into(),
            phase_rationale: "clear path".into(),
            steps: vec![StrategicStep {
                name: "n".into(),
                phase: "offense".into(),
                action: "a".into(),
                capability: "code_test".into(),
                resource_weight: 0.8,
            }],
            estimated_complexity: "high".into(),
        };
        let back: StrategicPlan = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back.current_phase, "offense");
        assert_eq!(back.steps[0].capability, "code_test");
    }
}
