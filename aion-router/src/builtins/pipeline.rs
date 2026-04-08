//! 管道类 builtin 技能：task_pipeline, task_race
//!
//! task_pipeline: 串行执行多个 capability，每步的输出作为下步的输入
//! task_race: 多个 capability 并行竞争，返回最先成功的结果

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::{BuiltinSkill, uuid_simple};

// ── task_pipeline ───────────────────────────────────────────────────────────

pub struct TaskPipeline;

#[async_trait::async_trait]
impl BuiltinSkill for TaskPipeline {
    fn name(&self) -> &'static str { "task_pipeline" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let steps: Vec<String> = context.context["steps"]
            .as_array()
            .ok_or_else(|| anyhow!("task_pipeline requires 'steps' array in context"))?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let initial_input = context.context["initial_input"]
            .as_str()
            .unwrap_or(&context.task)
            .to_string();

        if steps.is_empty() {
            return Err(anyhow!("task_pipeline: 'steps' array is empty"));
        }

        let registry = super::BuiltinRegistry::default_registry();
        let mut step_results = Vec::new();
        let mut current_input = initial_input.clone();

        for (i, cap) in steps.iter().enumerate() {
            let builtin_name = cap.as_str();

            // 构建当前步骤的 context
            let step_ctx = ExecutionContext {
                task: format!("{}: {}", cap, current_input),
                capability: cap.clone(),
                context: json!({"text": current_input, "input": current_input}),
                artifacts: Value::Object(Default::default()),
            };

            // 查找并执行 builtin
            let result = if let Some(builtin_impl) = registry.get(builtin_name) {
                let dummy_skill = SkillDefinition {
                    metadata: aion_types::types::SkillMetadata {
                        name: format!("pipeline_step_{}", i),
                        version: "0.1.0".to_string(),
                        capabilities: vec![cap.clone()],
                        entrypoint: format!("builtin:{}", cap),
                        permissions: aion_types::types::PermissionSet::default_deny(),
                        instruction: None,
                    },
                    root_dir: std::path::PathBuf::new(),
                    source: aion_types::types::SkillSource::Generated,
                };
                match builtin_impl.execute(&dummy_skill, &step_ctx).await {
                    Ok(val) => {
                        // 提取输出作为下一步的输入
                        current_input = val.get("text")
                            .or(val.get("output"))
                            .or(val.get("parsed"))
                            .and_then(|v| if v.is_string() { v.as_str().map(String::from) } else { Some(v.to_string()) })
                            .unwrap_or_else(|| serde_json::to_string(&val).unwrap_or_default());
                        json!({"step": i, "capability": cap, "status": "ok", "result": val})
                    }
                    Err(e) => {
                        json!({"step": i, "capability": cap, "status": "error", "error": e.to_string()})
                    }
                }
            } else {
                json!({"step": i, "capability": cap, "status": "skipped", "reason": format!("builtin '{}' not found", cap)})
            };

            let failed = result["status"] == "error";
            step_results.push(result);
            if failed {
                break; // 管道中断
            }
        }

        let all_ok = step_results.iter().all(|r| r["status"] == "ok");

        Ok(json!({
            "pipeline": steps,
            "initial_input": initial_input,
            "final_output": current_input,
            "step_count": steps.len(),
            "steps_executed": step_results.len(),
            "step_results": step_results,
            "status": if all_ok { "ok" } else { "partial" },
        }))
    }
}

// ── task_race ───────────────────────────────────────────────────────────────

pub struct TaskRace;

#[async_trait::async_trait]
impl BuiltinSkill for TaskRace {
    fn name(&self) -> &'static str { "task_race" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let task = context.context["task"]
            .as_str()
            .unwrap_or(&context.task)
            .to_string();
        let capability = context.context["capability"]
            .as_str()
            .unwrap_or("echo")
            .to_string();

        // 用不同的 builtin 竞争同一个任务
        // 如果指定了 agent_ids，按名称查找；否则用 capability 本身
        let candidates: Vec<String> = context.context.get("agent_ids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_else(|| vec![capability.clone()]);

        let race_id = uuid_simple();

        // 并行执行所有候选
        let mut set = tokio::task::JoinSet::new();
        for candidate in &candidates {
            let candidate = candidate.clone();
            let task = task.clone();
            set.spawn(async move {
                let registry = super::BuiltinRegistry::default_registry();
                let ctx = ExecutionContext {
                    task: format!("{}: {}", candidate, task),
                    capability: candidate.clone(),
                    context: json!({"text": task}),
                    artifacts: Value::Object(Default::default()),
                };
                let dummy_skill = SkillDefinition {
                    metadata: aion_types::types::SkillMetadata {
                        name: format!("race_{}", candidate),
                        version: "0.1.0".to_string(),
                        capabilities: vec![candidate.clone()],
                        entrypoint: format!("builtin:{}", candidate),
                        permissions: aion_types::types::PermissionSet::default_deny(),
                        instruction: None,
                    },
                    root_dir: std::path::PathBuf::new(),
                    source: aion_types::types::SkillSource::Generated,
                };
                let start = std::time::Instant::now();
                let result = if let Some(builtin) = registry.get(&candidate) {
                    builtin.execute(&dummy_skill, &ctx).await.ok()
                } else {
                    None
                };
                (candidate, result, start.elapsed())
            });
        }

        // 第一个成功的获胜
        let mut winner = None;
        let mut all_results = Vec::new();
        while let Some(Ok((name, result, duration))) = set.join_next().await {
            let entry = json!({
                "candidate": name,
                "duration_ms": duration.as_millis() as u64,
                "status": if result.is_some() { "ok" } else { "failed" },
            });
            if result.is_some() && winner.is_none() {
                winner = Some(json!({
                    "candidate": name,
                    "result": result,
                    "duration_ms": duration.as_millis() as u64,
                }));
            }
            all_results.push(entry);
        }

        Ok(json!({
            "race_id": race_id,
            "task": task,
            "candidates": candidates,
            "winner": winner,
            "all_results": all_results,
            "status": if winner.is_some() { "ok" } else { "no_winner" },
        }))
    }
}
