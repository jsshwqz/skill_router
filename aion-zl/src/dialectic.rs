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
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            THESIS_SYSTEM, &format!("Task: {}", task),
        ).await?;
        let thesis = parse_pos("thesis", &t);
        info!("Thesis done (confidence: {:.2})", thesis.confidence);

        // Antithesis
        info!("Phase 2/3: Antithesis...");
        let prompt = format!(
            "Task: {}\n\n--- THESIS ---\n{}\nStrengths: {:?}\nWeaknesses: {:?}",
            task, thesis.content, thesis.strengths, thesis.weaknesses
        );
        let a = ai::chat_json_deterministic(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            ANTITHESIS_SYSTEM, &prompt,
        ).await?;
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
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            SYNTHESIS_SYSTEM, &prompt,
        ).await?;
        let synthesis = parse_pos("synthesis", &s);
        info!("Synthesis done (confidence: {:.2})", synthesis.confidence);

        let _ = self.remember(
            &format!("Dialectic on '{}': T={:.2} A={:.2} S={:.2}",
                task, thesis.confidence, antithesis.confidence, synthesis.confidence),
            MemoryCategory::Decision,
        );

        Ok(DialecticalResult { task: task.into(), thesis, antithesis, synthesis, session_id })
    }
}

fn parse_pos(moment: &str, v: &serde_json::Value) -> Position {
    Position {
        moment: moment.into(),
        content: v["content"].as_str().unwrap_or("").into(),
        strengths: v["strengths"].as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        weaknesses: v["weaknesses"].as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        confidence: v["confidence"].as_f64().unwrap_or(0.5) as f32,
    }
}
