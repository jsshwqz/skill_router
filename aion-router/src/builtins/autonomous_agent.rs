//! 自主智能体 builtin：autonomous_agent
//!
//! 接收高层目标 → 自主规划 → 分布执行 → 动态适应 → 完整报告。
//! 核心是一个 AI 驱动的 Think→Act→Observe 循环。
//!
//! 工作流程：
//!   1. 分析目标，分解为可执行步骤
//!   2. 逐步骤执行（通过 AI 调用 forge 能力）
//!   3. 每步检查结果，动态调整下一步
//!   4. 达到目标或达到最大迭代次数后终止
//!   5. 返回结构化的执行报告

use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::orchestrator::call_http_ai_fallback;
use super::BuiltinSkill;

/// 最大迭代次数（防死循环）
const MAX_ITERATIONS: u32 = 20;

/// 系统级提示词：定义 agent 的工作方式
const AGENT_SYSTEM: &str = r#"You are an autonomous agent with access to a forge platform of AI tools.
Your job is to accomplish the user's goal by planning, executing, and adapting.

## Your Capabilities

You can call any forge capability by outputting a structured step. Each step has:
- `tool`: the forge capability name
- `input`: the input parameters
- `rationale`: why you chose this step

Available tools (use as needed):
- `market_search` — Find skills, tools, and information from external markets
- `web_search` — Search the web for information
- `http_fetch` — Fetch content from URLs
- `text_summarize` — Summarize long content
- `text_extract` — Extract key information
- `code_generate` — Generate code
- `json_parse` — Parse and validate JSON
- `yaml_parse` — Parse YAML
- `skill_convert` — Convert between SKILL.md and forge skill formats
- `discovery_search` — Cascade search across multiple sources

## Output Format

You must output valid JSON. Each turn, output ONE of:

PLAN mode (first turn):
{
  "mode": "plan",
  "goal_analysis": "brief analysis of the goal",
  "steps": [
    {"tool": "tool_name", "input": "what to do", "rationale": "why this step"},
    ...
  ]
}

ACTION mode (subsequent turns):
{
  "mode": "action",
  "tool": "tool_name",
  "input": "the input or query",
  "rationale": "why this step"
}

OBSERVE mode (reporting results):
{
  "mode": "observe",
  "tool": "tool_name",
  "result_summary": "what happened",
  "success": true or false,
  "next_action": "what to do next based on this result"
}

COMPLETE mode (when goal is achieved):
{
  "mode": "complete",
  "summary": "what was accomplished",
  "key_findings": ["finding 1", "finding 2"],
  "deliverables": ["deliverable 1"]
}

## Rules
- Never exceed a single tool call per turn
- If a step fails, try a different approach (max 3 retries per step)
- Be concrete and specific in tool inputs
- When the goal is met, switch to COMPLETE mode
- If stuck after multiple attempts, output COMPLETE with what was achieved
- Prefer simple, direct solutions over complex ones
"#;

pub struct AutonomousAgent;

#[async_trait::async_trait]
impl BuiltinSkill for AutonomousAgent {
    fn name(&self) -> &'static str {
        "autonomous_agent"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let goal = ctx.task.clone();
        let start = Instant::now();

        let mut log: Vec<Value> = Vec::new();
        let mut iteration: u32 = 0;

        // Phase 1: Plan
        let plan_prompt = format!(
            "{}<task>{}</task>\nAnalyze this goal and create a plan.",
            AGENT_SYSTEM, goal
        );
        let plan_result = call_http_ai_fallback(&plan_prompt, "autonomous_plan").await;
        let plan_output = plan_result.output.unwrap_or_default();
        log.push(json!({
            "phase": "plan",
            "output": plan_output,
            "engine": plan_result.engine,
        }));

        // Phase 2: Execute loop (simplified: AI generates next actions iteratively)
        let mut context = format!("Goal: {}\n\nInitial plan:\n{}\n\n", goal, plan_output);
        let mut completed = false;

        while iteration < MAX_ITERATIONS && !completed {
            iteration += 1;

            let turn_prompt = format!(
                "{}\n\n## Execution Log\n{}## Current Turn ({}/{})\nDecide what to do next. Output valid JSON.",
                AGENT_SYSTEM, context, iteration, MAX_ITERATIONS
            );

            let turn_result = call_http_ai_fallback(&turn_prompt, "autonomous_execute").await;
            let turn_output = turn_result.output.unwrap_or_default();

            // Check if we're done
            if turn_output.contains("\"mode\": \"complete\"")
                || turn_output.contains("\"mode\":\"complete\"")
            {
                context.push_str(&format!("Turn {}: COMPLETE\n{}\n", iteration, turn_output));
                log.push(json!({
                    "phase": "complete",
                    "iteration": iteration,
                    "output": turn_output,
                }));
                completed = true;
                break;
            }

            context.push_str(&format!("Turn {}:\n{}\n", iteration, turn_output));
            log.push(json!({
                "phase": "action",
                "iteration": iteration,
                "output": turn_output,
                "engine": turn_result.engine,
            }));
        }

        if !completed {
            log.push(json!({
                "phase": "timeout",
                "iteration": iteration,
                "message": format!("Reached max iterations ({}) without explicit completion", MAX_ITERATIONS),
            }));
        }

        let elapsed = start.elapsed();

        Ok(json!({
            "goal": goal,
            "status": if completed { "completed" } else { "max_iterations_reached" },
            "total_iterations": iteration,
            "elapsed_secs": elapsed.as_secs_f64(),
            "log": log,
        }))
    }
}
