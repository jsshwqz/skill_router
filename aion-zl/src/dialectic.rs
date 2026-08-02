//! 正反合 — Thesis-Antithesis-Synthesis
//! Based on: 《关于正确处理人民内部矛盾的问题》 "团结—批评—团结"

use crate::{ai, engine::Engine};
use aion_memory::memory::MemoryCategory;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

const THESIS_SYSTEM: &str = r#"You are a constructive solution architect.
Given a task, propose a concrete, actionable solution.

First, analyze the task requirements and constraints, then formulate your solution.

Output JSON:
{
  "content": "your proposed solution (detailed)",
  "strengths": ["strength1", "strength2"],
  "weaknesses": ["weakness1"],
  "confidence": 0.0-1.0
}

If the task is unclear or impossible, set confidence to 0 and explain in weaknesses. Do not fabricate a solution when the task is ambiguous — output confidence: 0 instead."#;

const ANTITHESIS_SYSTEM: &str = r#"You are a critical analyst and devil's advocate.
Given a task and a proposed solution (thesis), find flaws and propose an alternative.

First, identify specific weaknesses in the thesis. Point out at least one concrete flaw before providing your alternative. Be constructive but rigorous.

Output JSON:
{
  "content": "your alternative solution addressing thesis weaknesses",
  "strengths": ["strength1", "strength2"],
  "weaknesses": ["weakness1"],
  "confidence": 0.0-1.0
}

If you cannot find any flaws in the thesis, set confidence to 0 and explain why the thesis is sufficient."#;

const SYNTHESIS_SYSTEM: &str = r#"You are a dialectical synthesizer.
Given thesis and antithesis, create a synthesis that preserves strengths of both and resolves their contradictions.

First, compare the strengths and weaknesses of both positions side by side. Then construct a synthesis that:
1. Keeps the best elements from both
2. Addresses the weaknesses of each
3. Resolves contradictions between them

Be concrete and specific.

Output JSON:
{
  "content": "synthesized solution combining the best of both",
  "strengths": ["combined strength1"],
  "weaknesses": ["remaining limitation"],
  "confidence": 0.0-1.0
}

If the thesis and antithesis cannot be reconciled, set confidence < 0.3 and explain why in weaknesses. Do not force a synthesis when the two positions are fundamentally incompatible — output confidence: 0 and explain why."#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub moment: String,
    pub content: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialecticalResult {
    pub task: String,
    pub thesis: Position,
    pub antithesis: Position,
    pub synthesis: Position,
    pub session_id: String,
}

impl Engine {
    pub async fn task_dialectic(&self, task: &str) -> Result<DialecticalResult> {
        let session_id = uuid::Uuid::new_v4().to_string();
        info!(session = %session_id, "Starting dialectical process");

        // Thesis
        info!("Phase 1/3: Thesis...");
        let t = ai::chat_json_deterministic(
            &self.http,
            &self.ai_base_url,
            &self.ai_api_key,
            &self.ai_model,
            THESIS_SYSTEM,
            &format!("Task: {}", task),
        )
        .await?;
        let thesis = parse_pos("thesis", &t);
        info!("Thesis done (confidence: {:.2})", thesis.confidence);

        // Antithesis
        info!("Phase 2/3: Antithesis...");
        let prompt = format!(
            "Task: {}\n\n--- THESIS ---\n{}\nStrengths: {:?}\nWeaknesses: {:?}",
            task, thesis.content, thesis.strengths, thesis.weaknesses
        );
        let a = ai::chat_json_deterministic(
            &self.http,
            &self.ai_base_url,
            &self.ai_api_key,
            &self.ai_model,
            ANTITHESIS_SYSTEM,
            &prompt,
        )
        .await?;
        let antithesis = parse_pos("antithesis", &a);
        info!("Antithesis done (confidence: {:.2})", antithesis.confidence);

        // Synthesis
        info!("Phase 3/3: Synthesis...");
        let prompt = format!(
            "Task: {}\n\n--- THESIS ---\n{}\nStrengths: {:?}\nWeaknesses: {:?}\n\n--- ANTITHESIS ---\n{}\nStrengths: {:?}\nWeaknesses: {:?}",
            task, thesis.content, thesis.strengths, thesis.weaknesses,
            antithesis.content, antithesis.strengths, antithesis.weaknesses,
        );
        let s = ai::chat_json_deterministic(
            &self.http,
            &self.ai_base_url,
            &self.ai_api_key,
            &self.ai_model,
            SYNTHESIS_SYSTEM,
            &prompt,
        )
        .await?;
        let synthesis = parse_pos("synthesis", &s);
        info!("Synthesis done (confidence: {:.2})", synthesis.confidence);

        let _ = self.remember(
            &format!(
                "Dialectic on '{}': T={:.2} A={:.2} S={:.2}",
                task, thesis.confidence, antithesis.confidence, synthesis.confidence
            ),
            MemoryCategory::Decision,
        );

        Ok(build_dialectical_result(task, session_id, &t, &a, &s))
    }
}

/// Compose a [`DialecticalResult`] from the three AI passes' raw JSON.
fn build_dialectical_result(
    task: &str,
    session_id: String,
    thesis: &serde_json::Value,
    antithesis: &serde_json::Value,
    synthesis: &serde_json::Value,
) -> DialecticalResult {
    DialecticalResult {
        task: task.into(),
        thesis: parse_pos("thesis", thesis),
        antithesis: parse_pos("antithesis", antithesis),
        synthesis: parse_pos("synthesis", synthesis),
        session_id,
    }
}

fn parse_pos(moment: &str, v: &serde_json::Value) -> Position {
    Position {
        moment: moment.into(),
        content: v["content"].as_str().unwrap_or("").into(),
        strengths: v["strengths"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        weaknesses: v["weaknesses"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        confidence: v["confidence"].as_f64().unwrap_or(0.5) as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn position_json(content: &str, strengths: &[&str], weaknesses: &[&str], confidence: f64) -> serde_json::Value {
        json!({
            "content": content,
            "strengths": strengths,
            "weaknesses": weaknesses,
            "confidence": confidence
        })
    }

    #[test]
    fn parse_pos_maps_all_fields() {
        let v = position_json("proposed solution", &["a", "b"], &["c"], 0.9);
        let p = parse_pos("thesis", &v);
        assert_eq!(p.moment, "thesis");
        assert_eq!(p.content, "proposed solution");
        assert_eq!(p.strengths, vec!["a", "b"]);
        assert_eq!(p.weaknesses, vec!["c"]);
        assert_eq!(p.confidence, 0.9);
    }

    #[test]
    fn parse_pos_uses_defaults_for_missing_fields() {
        let p = parse_pos("antithesis", &json!({}));
        assert_eq!(p.moment, "antithesis");
        assert_eq!(p.content, "");
        assert!(p.strengths.is_empty());
        assert!(p.weaknesses.is_empty());
        assert_eq!(p.confidence, 0.5);
    }

    #[test]
    fn dialectical_result_has_thesis_antithesis_synthesis_structure() {
        let t = position_json("do A", &["fast"], &["risky"], 0.8);
        let a = position_json("do B", &["safe"], &["slow"], 0.6);
        let s = position_json("do A then B", &["fast", "safe"], &["complex"], 0.9);

        let result = build_dialectical_result("task", "session-1".into(), &t, &a, &s);
        assert_eq!(result.task, "task");
        assert_eq!(result.session_id, "session-1");
        assert_eq!(result.thesis.moment, "thesis");
        assert_eq!(result.antithesis.moment, "antithesis");
        assert_eq!(result.synthesis.moment, "synthesis");
        assert_eq!(result.thesis.content, "do A");
        assert_eq!(result.antithesis.content, "do B");
        assert_eq!(result.synthesis.content, "do A then B");
        assert_eq!(result.synthesis.confidence, 0.9);
    }

    #[test]
    fn dialectical_result_serde_roundtrip() {
        let t = position_json("t", &["x"], &["y"], 0.7);
        let a = position_json("a", &[], &["z"], 0.4);
        let s = position_json("s", &["x", "z"], &[], 0.8);
        let result = build_dialectical_result("task", "uuid-123".into(), &t, &a, &s);

        let back: DialecticalResult = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(back.task, "task");
        assert_eq!(back.session_id, "uuid-123");
        assert_eq!(back.thesis.moment, "thesis");
        assert_eq!(back.synthesis.moment, "synthesis");
        assert_eq!(back.antithesis.weaknesses, vec!["z"]);
    }
}
