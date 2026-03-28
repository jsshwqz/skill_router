//! Crew 执行引擎
//!
//! 按拓扑顺序执行 CrewConfig 中定义的多角色任务流。

use std::collections::HashMap;

use anyhow::Result;
use serde_json::{json, Value};

use aion_types::agent_template::{AgentTemplate, CrewConfig};

use crate::SkillRouter;

/// Crew 执行结果
#[derive(Debug, Clone)]
pub struct CrewResult {
    /// 每个任务的执行结果
    pub task_results: Vec<TaskResult>,
    /// 是否全部成功
    pub success: bool,
}

/// 单个任务执行结果
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// 任务名称
    pub task_name: String,
    /// 执行的 Agent 名称
    pub agent_name: String,
    /// 执行结果
    pub output: Value,
    /// 是否成功
    pub success: bool,
}

/// Crew 执行引擎
pub struct CrewExecutor;

impl CrewExecutor {
    /// 执行 Crew 配置
    pub async fn run(
        config: &CrewConfig,
        variables: &HashMap<String, String>,
        router: &SkillRouter,
    ) -> Result<CrewResult> {
        config.validate()?;

        let mut task_results = Vec::new();
        let mut context_vars = variables.clone();

        for task in config.execution_order() {
            let agent = config.get_agent(&task.agent).ok_or_else(|| {
                anyhow::anyhow!("agent '{}' not found for task '{}'", task.agent, task.name)
            })?;

            tracing::info!(
                task = %task.name,
                agent = %agent.name,
                role = %agent.role,
                "crew: executing task"
            );

            // 插值任务描述
            let description = Self::interpolate(&task.description, &context_vars);

            // 构建包含角色上下文的执行请求
            let system_prompt = agent.system_prompt(&context_vars);

            // 通过 SkillRouter 执行
            let result = Self::execute_with_agent(
                router,
                agent,
                &description,
                &system_prompt,
                &context_vars,
            )
            .await;

            match result {
                Ok(output) => {
                    // 如果任务有输出变量名，存入上下文供后续任务使用
                    if let Some(ref output_name) = task.output {
                        let output_str = output["result"]
                            .as_str()
                            .unwrap_or(&output.to_string())
                            .to_string();
                        context_vars.insert(output_name.clone(), output_str);
                    }

                    task_results.push(TaskResult {
                        task_name: task.name.clone(),
                        agent_name: agent.name.clone(),
                        output,
                        success: true,
                    });
                }
                Err(e) => {
                    tracing::error!(
                        task = %task.name,
                        error = %e,
                        "crew: task failed"
                    );
                    task_results.push(TaskResult {
                        task_name: task.name.clone(),
                        agent_name: agent.name.clone(),
                        output: json!({"error": e.to_string()}),
                        success: false,
                    });

                    // 任务失败，整个 Crew 终止
                    return Ok(CrewResult {
                        success: false,
                        task_results,
                    });
                }
            }
        }

        Ok(CrewResult {
            success: true,
            task_results,
        })
    }

    /// 用特定 Agent 身份执行任务
    async fn execute_with_agent(
        router: &SkillRouter,
        agent: &AgentTemplate,
        task_description: &str,
        system_prompt: &str,
        _variables: &HashMap<String, String>,
    ) -> Result<Value> {
        // 选择最匹配的能力
        let _capability = if !agent.capabilities.is_empty() {
            // 用 AI 从 agent 的能力范围中选择最佳匹配
            agent.capabilities[0].clone()
        } else {
            "ai_task".to_string()
        };

        let context = json!({
            "text": task_description,
            "system_prompt": system_prompt,
            "agent_role": agent.role,
            "agent_goal": agent.goal,
        });

        let result = router
            .route_with_context(task_description, Some(context))
            .await?;

        Ok(json!({
            "task": task_description,
            "agent": agent.name,
            "capability": result.capability,
            "result": result.execution.result,
            "status": result.execution.status
        }))
    }

    /// 模板变量插值
    fn interpolate(template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate() {
        let mut vars = HashMap::new();
        vars.insert("topic".to_string(), "Rust".to_string());
        vars.insert("depth".to_string(), "deep".to_string());

        let result = CrewExecutor::interpolate(
            "Research {topic} at {depth} level",
            &vars,
        );
        assert_eq!(result, "Research Rust at deep level");
    }

    #[test]
    fn test_crew_result_structure() {
        let result = CrewResult {
            success: true,
            task_results: vec![
                TaskResult {
                    task_name: "t1".to_string(),
                    agent_name: "a1".to_string(),
                    output: json!({"result": "done"}),
                    success: true,
                },
            ],
        };
        assert!(result.success);
        assert_eq!(result.task_results.len(), 1);
    }
}
