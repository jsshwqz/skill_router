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

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, PermissionSet, SkillDefinition, SkillMetadata, SkillSource};

use super::orchestrator::call_http_ai_fallback;
use super::{BuiltinRegistry, BuiltinSkill};

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
- `echo` — Verify real tool execution and pass data through unchanged
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
- `record_change` — Record each changed file after implementation
- `record_decision` — Record architecture and design choices
- `session_report` — Generate the final self-evolution report
- `memory_remember` — Persist reusable lessons and decisions

Self-evolution tool argument schemas:
- `record_change`: {"kind":"feature|fix|refactor|prompt|doc|config|test","file":"path","summary":"change summary"}
- `record_decision`: {"context":"decision context","choice":"selected option","rationale":"reason for selection"}
- `session_report`: {}
- `memory_remember`: {"category":"Decision","content":"reusable lesson or decision"}

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
  "arguments": {"tool_specific_field": "value"},
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
- ACTION mode executes the named tool for real; never invent an OBSERVE result
- After each ACTION, use the actual observation supplied in the execution log
- For implementation work, finish with record_change, record_decision, and session_report
- If a step fails, try a different approach (max 3 retries per step)
- Be concrete and specific in tool inputs
- When the goal is met, switch to COMPLETE mode
- If stuck after multiple attempts, output COMPLETE with what was achieved
- Prefer simple, direct solutions over complex ones
"#;

pub struct AutonomousAgent;

#[derive(Debug, Deserialize)]
struct AgentAction {
    mode: String,
    tool: Option<String>,
    arguments: Option<Value>,
    input: Option<Value>,
}

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
        let registry = BuiltinRegistry::default_registry();

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
            if turn_output.contains("\"mode\": \"complete\"") || turn_output.contains("\"mode\":\"complete\"") {
                context.push_str(&format!("Turn {}: COMPLETE\n{}\n", iteration, turn_output));
                log.push(json!({
                    "phase": "complete",
                    "iteration": iteration,
                    "output": turn_output,
                }));
                completed = true;
                break;
            }

            match parse_action(&turn_output) {
                Ok(action) if action.mode == "action" => {
                    let tool = action
                        .tool
                        .clone()
                        .ok_or_else(|| anyhow!("autonomous action missing tool"))?;
                    let arguments = action_arguments(action)?;
                    let result = execute_action(&registry, &tool, arguments).await;
                    let observation = match &result {
                        Ok(value) => bounded_observation(value.to_string()),
                        Err(error) => format!("error: {error}"),
                    };
                    context.push_str(&format!(
                        "Turn {iteration}: ACTION {tool}\nActual observation:\n{observation}\n"
                    ));
                    log.push(json!({
                        "phase": "action",
                        "iteration": iteration,
                        "tool": tool,
                        "success": result.is_ok(),
                        "observation": observation,
                        "engine": turn_result.engine,
                    }));
                }
                Ok(action) => {
                    context.push_str(&format!(
                        "Turn {iteration}: invalid mode '{}'; output ACTION or COMPLETE.\n",
                        action.mode
                    ));
                }
                Err(error) => {
                    context.push_str(&format!(
                        "Turn {iteration}: invalid action JSON ({error}); output ACTION or COMPLETE.\n"
                    ));
                }
            }
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

fn parse_action(output: &str) -> Result<AgentAction> {
    let trimmed = output.trim();
    let json_text = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    serde_json::from_str(json_text).map_err(Into::into)
}

fn action_arguments(action: AgentAction) -> Result<Value> {
    if let Some(arguments) = action.arguments {
        if arguments.is_object() {
            return Ok(arguments);
        }
        bail!("autonomous action arguments must be an object");
    }
    match action.input {
        Some(Value::String(input)) => Ok(json!({"task": input, "text": input})),
        Some(value) if value.is_object() => Ok(value),
        _ => Ok(json!({})),
    }
}

async fn execute_action(registry: &BuiltinRegistry, tool: &str, arguments: Value) -> Result<Value> {
    if tool == "autonomous_agent" || tool == "ai_task" {
        bail!("recursive autonomous tool call is not allowed");
    }
    let builtin = registry
        .get(tool)
        .ok_or_else(|| anyhow!("unknown Forge tool '{tool}'"))?;
    let task = arguments
        .get("task")
        .or_else(|| arguments.get("text"))
        .or_else(|| arguments.get("input"))
        .and_then(Value::as_str)
        .unwrap_or(tool)
        .to_string();
    let skill = SkillDefinition {
        metadata: SkillMetadata {
            name: tool.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec![tool.to_string()],
            entrypoint: format!("builtin:{tool}"),
            permissions: PermissionSet::default_deny().with_network(true),
            instruction: None,
            engine_capable: false,
        },
        root_dir: std::env::current_dir().unwrap_or_default(),
        source: SkillSource::Local,
    };
    let context = ExecutionContext::new(&task, tool).with_context(arguments);
    builtin.execute(&skill, &context).await
}

fn bounded_observation(mut observation: String) -> String {
    const LIMIT: usize = 8 * 1024;
    if observation.len() <= LIMIT {
        return observation;
    }
    let mut end = LIMIT;
    while !observation.is_char_boundary(end) {
        end -= 1;
    }
    observation.truncate(end);
    observation.push_str("...[truncated]");
    observation
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{action_arguments, execute_action, parse_action, AgentAction, BuiltinRegistry};

    #[test]
    fn parses_structured_actions_and_legacy_string_input() {
        let action =
            parse_action("```json\n{\"mode\":\"action\",\"tool\":\"echo\",\"arguments\":{\"text\":\"ok\"}}\n```")
                .unwrap();
        assert_eq!(action.tool.as_deref(), Some("echo"));
        assert_eq!(action_arguments(action).unwrap()["text"], "ok");

        let legacy = AgentAction {
            mode: "action".to_string(),
            tool: Some("echo".to_string()),
            arguments: None,
            input: Some(json!("legacy")),
        };
        assert_eq!(action_arguments(legacy).unwrap()["task"], "legacy");
    }

    #[tokio::test]
    async fn executes_real_tools_and_blocks_recursive_calls() {
        let registry = BuiltinRegistry::default_registry();
        let result = execute_action(&registry, "echo", json!({"text": "actual"}))
            .await
            .unwrap();
        assert_eq!(result["echo"], "actual");
        assert!(execute_action(&registry, "autonomous_agent", json!({}))
            .await
            .unwrap_err()
            .to_string()
            .contains("recursive"));
    }

    #[tokio::test]
    async fn rejects_incomplete_self_evolution_records() {
        let registry = BuiltinRegistry::default_registry();
        let decision_error = execute_action(
            &registry,
            "record_decision",
            json!({"context": "", "choice": "", "rationale": ""}),
        )
        .await
        .unwrap_err();
        assert!(decision_error.to_string().contains("non-empty"));

        let change_error = execute_action(
            &registry,
            "record_change",
            json!({"kind": "unknown", "file": "", "summary": ""}),
        )
        .await
        .unwrap_err();
        assert!(change_error.to_string().contains("kind must be"));
    }
}
