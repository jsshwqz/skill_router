//! 实践论 — Dialectical retry with learning
//! 实践 → 感性认识 → 理性认识 → 再实践

use crate::{ai, engine::Engine};
use aion_memory::memory::MemoryCategory;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const ROOT_CAUSE_SYSTEM: &str = r#"You are an expert at root cause analysis.
Given a task, strategy used, and error, analyze:

First, distinguish symptoms from root cause. Symptoms are surface-level manifestations; root cause is the underlying reason.
Then:
1. Identify the root cause (not just symptoms)
2. Extract a lesson learned
3. Propose an alternative strategy to try next

Output JSON:
{
  "root_cause": "...",
  "lesson": "...",
  "next_strategy": "concrete alternative approach"
}

If you cannot determine the root cause with confidence, set root_cause to "unknown"."#;

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
        let result = retry_loop(
            task,
            max,
            |strategy: &str| {
                let engine = self;
                let strategy = strategy.to_string();
                async move {
                    match engine.route(&strategy).await {
                        Ok(res) if res.execution.error.is_none() => match serde_json::to_value(&res.execution) {
                            Ok(v) => Ok(v),
                            Err(e) => Err(e.to_string()),
                        },
                        Ok(res) => Err(res.execution.error.unwrap_or_else(|| "unknown".into())),
                        Err(e) => Err(e.to_string()),
                    }
                }
            },
            |task: &str, strategy: &str, err: &str| {
                let engine = self;
                let task = task.to_string();
                let strategy = strategy.to_string();
                let err = err.to_string();
                async move { engine.analyze_failure(&task, &strategy, &err).await }
            },
        )
        .await;

        // 成功/失败的学习记忆（内容与原实现一致，仅调用时机移到结果返回后）
        if result.success {
            let _ = self.remember(
                &format!("Task '{}' succeeded with: {}", task, result.final_strategy),
                MemoryCategory::Lesson,
            );
        } else {
            let _ = self.remember(
                &format!("Task '{}' failed after {} attempts", task, result.total_attempts),
                MemoryCategory::Error,
            );
        }

        Ok(result)
    }

    async fn analyze_failure(&self, task: &str, strategy: &str, error: &str) -> (String, String, String) {
        let memories = self.recall(task).unwrap_or_default();
        let mem_hint = if memories.is_empty() {
            String::new()
        } else {
            let s: Vec<_> = memories.iter().map(|m| m.content.as_str()).collect();
            format!("\nPrior lessons: {}", s.join("; "))
        };

        let prompt = format!("Task: {}\nStrategy: {}\nError: {}{}", task, strategy, error, mem_hint);

        match ai::chat_json_deterministic(
            &self.http,
            &self.ai_base_url,
            &self.ai_api_key,
            &self.ai_model,
            ROOT_CAUSE_SYSTEM,
            &prompt,
        )
        .await
        {
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

/// Core dialectical retry loop, with the execution and failure-analysis steps
/// injected so it can be unit-tested without a network or an [`Engine`].
///
/// - `execute(&strategy)` returns `Ok(value)` on success (retries stop) or
///   `Err(error)` on failure (the error is recorded and analyzed).
/// - `analyze(task, strategy, error)` returns `(root_cause, lesson, next_strategy)`;
///   the returned `next_strategy` is used for the following attempt.
async fn retry_loop<F, Fut, G, GFut>(
    task: &str,
    max: u32,
    mut execute: F,
    mut analyze: G,
) -> RetryResult
where
    F: FnMut(&str) -> Fut,
    Fut: std::future::Future<Output = std::result::Result<serde_json::Value, String>>,
    G: FnMut(&str, &str, &str) -> GFut,
    GFut: std::future::Future<Output = (String, String, String)>,
{
    let max = if max == 0 { 3 } else { max };
    let mut attempts = Vec::new();
    let mut strategy = task.to_string();

    for n in 1..=max {
        info!(attempt = n, "Executing: {}", strategy);

        match execute(&strategy).await {
            Ok(value) => {
                info!(attempt = n, "Success!");
                return RetryResult {
                    task: task.into(),
                    success: true,
                    final_result: Some(value),
                    attempts,
                    total_attempts: n,
                    final_strategy: strategy,
                };
            }
            Err(err) => {
                warn!(attempt = n, error = %err, "Failed");
                let (rc, lesson, next) = analyze(task, &strategy, &err).await;
                attempts.push(RetryAttempt {
                    attempt: n,
                    strategy: strategy.clone(),
                    error: err,
                    root_cause: Some(rc),
                    lesson: Some(lesson),
                });
                strategy = next;
            }
        }
    }

    RetryResult {
        task: task.into(),
        success: false,
        final_result: None,
        attempts,
        total_attempts: max,
        final_strategy: strategy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn failing_execute(
        failures: u32,
    ) -> impl FnMut(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::result::Result<serde_json::Value, String>> + Send>>
    {
        let counter = Arc::new(AtomicU32::new(0));
        move |strategy: &str| {
            let counter = counter.clone();
            let s = strategy.to_string();
            Box::pin(async move {
                if counter.fetch_add(1, Ordering::SeqCst) < failures {
                    Err(format!("boom at {}", s))
                } else {
                    Ok(serde_json::json!({ "ok": true }))
                }
            })
        }
    }

    fn next_analyzer(
    ) -> impl FnMut(&str, &str, &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = (String, String, String)> + Send>>
    {
        let counter = Arc::new(AtomicU32::new(0));
        move |_task: &str, _strategy: &str, err: &str| {
            let counter = counter.clone();
            let err = err.to_string();
            Box::pin(async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                (
                    format!("root cause #{}", n),
                    format!("lesson {}", err),
                    format!("next-{}", n),
                )
            })
        }
    }

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let result = retry_loop(
            "task",
            3,
            |_| Box::pin(async { Ok(serde_json::json!({ "ok": true })) }),
            next_analyzer(),
        )
        .await;

        assert!(result.success);
        assert_eq!(result.total_attempts, 1);
        assert!(result.attempts.is_empty(), "no failures to record");
        assert_eq!(result.final_strategy, "task");
        assert_eq!(result.final_result, Some(serde_json::json!({ "ok": true })));
    }

    #[tokio::test]
    async fn retry_fails_twice_then_succeeds_and_switches_strategy() {
        let result = retry_loop("task", 3, failing_execute(2), next_analyzer()).await;

        assert!(result.success, "third attempt should succeed");
        assert_eq!(result.total_attempts, 3);
        assert_eq!(result.attempts.len(), 2, "two failed attempts recorded");

        // 第 1 次失败使用初始策略（即任务本身）
        assert_eq!(result.attempts[0].attempt, 1);
        assert_eq!(result.attempts[0].strategy, "task");
        assert_eq!(result.attempts[0].error, "boom at task");
        assert_eq!(result.attempts[0].root_cause.as_deref(), Some("root cause #0"));
        assert_eq!(result.attempts[0].lesson.as_deref(), Some("lesson boom at task"));

        // 第 2 次失败已切换为第 1 次根因分析建议的策略
        assert_eq!(result.attempts[1].attempt, 2);
        assert_eq!(result.attempts[1].strategy, "next-0");
        assert_eq!(result.attempts[1].error, "boom at next-0");

        // 成功尝试使用第 2 次分析建议的策略
        assert_eq!(result.final_strategy, "next-1");
        assert_eq!(result.final_result, Some(serde_json::json!({ "ok": true })));
    }

    #[tokio::test]
    async fn retry_exhausts_attempts_and_reports_failure() {
        let result = retry_loop("task", 4, failing_execute(99), next_analyzer()).await;

        assert!(!result.success);
        assert_eq!(result.total_attempts, 4);
        assert_eq!(result.attempts.len(), 4, "every attempt failed and was recorded");
        assert!(result.final_result.is_none());
        assert_eq!(result.attempts[3].strategy, "next-2");
        assert_eq!(result.final_strategy, "next-3");
    }

    #[tokio::test]
    async fn retry_zero_max_defaults_to_three() {
        let result = retry_loop("task", 0, failing_execute(99), next_analyzer()).await;
        assert!(!result.success);
        assert_eq!(result.total_attempts, 3);
        assert_eq!(result.attempts.len(), 3);
    }

    #[tokio::test]
    async fn retry_single_attempt_failure() {
        let result = retry_loop("task", 1, failing_execute(99), next_analyzer()).await;
        assert!(!result.success);
        assert_eq!(result.total_attempts, 1);
        assert_eq!(result.attempts.len(), 1);
        assert_eq!(result.final_strategy, "next-0");
    }

    #[test]
    fn retry_attempt_and_result_serde_roundtrip() {
        let attempt = RetryAttempt {
            attempt: 2,
            strategy: "next-1".into(),
            error: "boom".into(),
            root_cause: Some("root cause".into()),
            lesson: Some("lesson".into()),
        };
        let result = RetryResult {
            task: "task".into(),
            success: false,
            final_result: None,
            attempts: vec![attempt],
            total_attempts: 2,
            final_strategy: "next-1".into(),
        };
        let back: RetryResult = serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(back.total_attempts, 2);
        assert_eq!(back.attempts[0].strategy, "next-1");
        assert_eq!(back.attempts[0].root_cause.as_deref(), Some("root cause"));
    }
}
