//! Agent 模板与 Crew 配置
//!
//! CrewAI 风格的 YAML 配置驱动的角色化 Agent 系统。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Agent 模板定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplate {
    /// Agent 名称
    pub name: String,
    /// 角色标题
    pub role: String,
    /// 目标
    pub goal: String,
    /// 背景故事（注入为系统提示词）
    #[serde(default)]
    pub backstory: String,
    /// 可用能力列表
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// AI 模型覆盖
    #[serde(default)]
    pub model: Option<String>,
    /// 是否允许委派任务给其他 Agent
    #[serde(default)]
    pub allow_delegation: bool,
}

impl AgentTemplate {
    /// 生成系统提示词
    pub fn system_prompt(&self, variables: &HashMap<String, String>) -> String {
        let role = Self::interpolate(&self.role, variables);
        let goal = Self::interpolate(&self.goal, variables);
        let backstory = Self::interpolate(&self.backstory, variables);

        format!(
            "你的角色是：{}\n\
             你的目标是：{}\n\
             背景：{}\n\
             你可以使用的能力：{}\n\
             请根据你的角色和目标完成任务。",
            role,
            goal,
            backstory,
            self.capabilities.join(", ")
        )
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

/// 任务模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: String,
    /// 执行此任务的 Agent 名称
    pub agent: String,
    /// 依赖的前置任务
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// 输出变量名（供后续任务引用）
    #[serde(default)]
    pub output: Option<String>,
}

/// Crew 配置（多角色协作编排）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewConfig {
    /// Agent 模板列表
    pub agents: Vec<AgentTemplate>,
    /// 任务列表
    pub tasks: Vec<TaskTemplate>,
}

impl CrewConfig {
    /// 从 JSON 解析 Crew 配置
    pub fn from_json(json: &serde_json::Value) -> anyhow::Result<Self> {
        // 解析 agents
        let agents_map = json["agents"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("missing 'agents' section"))?;

        let mut agents = Vec::new();
        for (name, def) in agents_map {
            let agent = AgentTemplate {
                name: name.clone(),
                role: def["role"].as_str().unwrap_or("").to_string(),
                goal: def["goal"].as_str().unwrap_or("").to_string(),
                backstory: def["backstory"].as_str().unwrap_or("").to_string(),
                capabilities: def["capabilities"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                model: def["model"].as_str().map(String::from),
                allow_delegation: def["allow_delegation"].as_bool().unwrap_or(false),
            };
            agents.push(agent);
        }

        // 解析 tasks
        let tasks_map = json["tasks"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("missing 'tasks' section"))?;

        let mut tasks = Vec::new();
        for (name, def) in tasks_map {
            let task = TaskTemplate {
                name: name.clone(),
                description: def["description"].as_str().unwrap_or("").to_string(),
                agent: def["agent"].as_str().unwrap_or("").to_string(),
                depends_on: def["depends_on"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                output: def["output"].as_str().map(String::from),
            };
            tasks.push(task);
        }

        Ok(Self { agents, tasks })
    }

    /// 验证配置完整性
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.agents.is_empty() {
            return Err(anyhow::anyhow!("crew config has no agents"));
        }
        if self.tasks.is_empty() {
            return Err(anyhow::anyhow!("crew config has no tasks"));
        }

        // 检查所有任务引用的 agent 存在
        let agent_names: Vec<&str> = self.agents.iter().map(|a| a.name.as_str()).collect();
        for task in &self.tasks {
            if !agent_names.contains(&task.agent.as_str()) {
                return Err(anyhow::anyhow!(
                    "task '{}' references unknown agent '{}'",
                    task.name,
                    task.agent
                ));
            }
        }

        // 检查任务依赖引用的任务存在
        let task_names: Vec<&str> = self.tasks.iter().map(|t| t.name.as_str()).collect();
        for task in &self.tasks {
            for dep in &task.depends_on {
                if !task_names.contains(&dep.as_str()) {
                    return Err(anyhow::anyhow!(
                        "task '{}' depends on unknown task '{}'",
                        task.name,
                        dep
                    ));
                }
            }
        }

        // 检查无循环依赖（简单检查：依赖的任务必须排在前面）
        for (i, task) in self.tasks.iter().enumerate() {
            for dep in &task.depends_on {
                let dep_idx = self.tasks.iter().position(|t| t.name == *dep);
                if let Some(idx) = dep_idx {
                    if idx >= i {
                        return Err(anyhow::anyhow!(
                            "task '{}' depends on '{}' which comes later (possible cycle)",
                            task.name,
                            dep
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取拓扑排序后的任务执行顺序
    pub fn execution_order(&self) -> Vec<&TaskTemplate> {
        // 已按声明顺序排列（validate 确保了依赖在前）
        self.tasks.iter().collect()
    }

    /// 获取指定名称的 Agent 模板
    pub fn get_agent(&self, name: &str) -> Option<&AgentTemplate> {
        self.agents.iter().find(|a| a.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_config_json() -> serde_json::Value {
        json!({
            "agents": {
                "researcher": {
                    "role": "研究员",
                    "goal": "调研{topic}",
                    "backstory": "你是专家",
                    "capabilities": ["web_search", "http_fetch"],
                    "model": "qwen2.5:7b"
                },
                "writer": {
                    "role": "写手",
                    "goal": "撰写报告",
                    "backstory": "你擅长写作",
                    "capabilities": ["text_summarize"],
                    "allow_delegation": true
                }
            },
            "tasks": {
                "research": {
                    "description": "调研{topic}最新进展",
                    "agent": "researcher",
                    "output": "research_result"
                },
                "write": {
                    "description": "基于研究撰写报告",
                    "agent": "writer",
                    "depends_on": ["research"]
                }
            }
        })
    }

    #[test]
    fn test_crew_config_parse() {
        let config = CrewConfig::from_json(&sample_config_json()).unwrap();
        assert_eq!(config.agents.len(), 2);
        assert_eq!(config.tasks.len(), 2);
        assert_eq!(config.agents[0].capabilities.len(), 2);
    }

    #[test]
    fn test_crew_config_validate() {
        let config = CrewConfig::from_json(&sample_config_json()).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_crew_config_validate_missing_agent() {
        let json = json!({
            "agents": {
                "a": { "role": "r", "goal": "g" }
            },
            "tasks": {
                "t": { "description": "d", "agent": "nonexistent" }
            }
        });
        let config = CrewConfig::from_json(&json).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_agent_template_system_prompt() {
        let agent = AgentTemplate {
            name: "test".to_string(),
            role: "{topic}研究员".to_string(),
            goal: "调研{topic}".to_string(),
            backstory: "专家".to_string(),
            capabilities: vec!["web_search".to_string()],
            model: None,
            allow_delegation: false,
        };

        let mut vars = HashMap::new();
        vars.insert("topic".to_string(), "AI安全".to_string());

        let prompt = agent.system_prompt(&vars);
        assert!(prompt.contains("AI安全研究员"));
        assert!(prompt.contains("调研AI安全"));
    }

    #[test]
    fn test_interpolate() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Alice".to_string());
        vars.insert("topic".to_string(), "Rust".to_string());

        let result = AgentTemplate::interpolate("Hello {name}, let's discuss {topic}", &vars);
        assert_eq!(result, "Hello Alice, let's discuss Rust");
    }

    #[test]
    fn test_execution_order() {
        let config = CrewConfig::from_json(&sample_config_json()).unwrap();
        let order = config.execution_order();
        assert_eq!(order[0].name, "research");
        assert_eq!(order[1].name, "write");
    }
}
