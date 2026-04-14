//! 实践论 — Dialectical retry with learning
//! 实践 → 感性认识 → 理性认识 → 再实践

use crate::{ai, engine::Engine};
use aion_memory::memory::MemoryCategory;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const ROOT_CAUSE_SYSTEM: &str = r#"You are an expert at root cause analysis.
Given a task, strategy used, and error, analyze:
1. Root cause (not just symptoms)
2. Lesson learned
3. Alternative strategy to try next

Output JSON:
{
  "root_cause": "...",
  "lesson": "...",
  "next_strategy": "concrete alternative approach"
}"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttempt {
    pub attempt: u32,
    pub strategy: String,
    pub error: String,
    pub root_cause: Option<String>,
    pub lesson: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryResult {
    pub task: String,
    pub success: bool,
    pub final_result: Option<serde_json::Value>,
    pub attempts: Vec<RetryAttempt>,
    pub total_attempts: u32,
    pub final_strategy: String,
}

impl Engine {
    pub async fn dialectical_retry(&self, task: &str, max: u32) -> Result<RetryResult> {
        let max = if max == 0 { 3 } else { max };
        let mut attempts = Vec::new();
        let mut strategy = task.to_string();

        for n in 1..=max {
            info!(attempt = n, "Executing: {}", strategy);

            match self.route(&strategy).await {
                Ok(result) if result.execution.error.is_none() => {
                    info!(attempt = n, "Success!");
                    let _ = self.remember(
                        &format!("Task '{}' succeeded with: {}", task, strategy),
                        MemoryCategory::Lesson,
                    );
                    return Ok(RetryResult {
                        task: task.into(),
                        success: true,
                        final_result: Some(serde_json::to_value(&result.execution)?),
                        attempts,
                        total_attempts: n,
                        final_strategy: strategy,
                    });
                }
                Ok(result) => {
                    let err = result.execution.error.unwrap_or_else(|| "unknown".into());
                    warn!(attempt = n, error = %err, "Failed");
                    let (rc, lesson, next) = self.analyze_failure(task, &strategy, &err).await;
                    attempts.push(RetryAttempt {
                        attempt: n, strategy: strategy.clone(), error: err,
                        root_cause: Some(rc), lesson: Some(lesson),
                    });
                    strategy = next;
                }
                Err(e) => {
                    let err = e.to_string();
                    warn!(attempt = n, error = %err, "Route error");
                    let (rc, lesson, next) = self.analyze_failure(task, &strategy, &err).await;
                    attempts.push(RetryAttempt {
                        attempt: n, strategy: strategy.clone(), error: err,
                        root_cause: Some(rc), lesson: Some(lesson),
                    });
                    strategy = next;
                }
            }
        }

        let _ = self.remember(
            &format!("Task '{}' failed after {} attempts", task, max),
            MemoryCategory::Error,
        );

        Ok(RetryResult {
            task: task.into(), success: false, final_result: None,
            attempts, total_attempts: max, final_strategy: strategy,
        })
    }

    async fn analyze_failure(&self, task: &str, strategy: &str, error: &str)
        -> (String, String, String)
    {
        let memories = self.recall(task).unwrap_or_default();
        let mem_hint = if memories.is_empty() { String::new() }
        else {
            let s: Vec<_> = memories.iter().map(|m| m.content.as_str()).collect();
            format!("\nPrior lessons: {}", s.join("; "))
        };

        let prompt = format!("Task: {}\nStrategy: {}\nError: {}{}", task, strategy, error, mem_hint);

        match ai::chat_json(
            &self.http, &self.ai_base_url, &self.ai_api_key, &self.ai_model,
            ROOT_CAUSE_SYSTEM, &prompt,
        ).await {
            Ok(v) => {
                let rc = v["root_cause"].as_str().unwrap_or("unknown").to_string();
                let lesson = v["lesson"].as_str().unwrap_or("").to_string();
                let next = v["next_strategy"].as_str().unwrap_or(task).to_string();
                if !lesson.is_empty() {
                    let _ = self.remember(&lesson, MemoryCategory::Lesson);
                }
                (rc, lesson, next)
            }
            Err(_) => ("AI unavailable".into(), String::new(), task.into()),
        }
    }
}
