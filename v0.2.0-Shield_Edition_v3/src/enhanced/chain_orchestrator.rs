use crate::models::{SkillMetadata, SubTask, TaskStatus, SubTaskResult, ExecutionResult};
use std::collections::{HashMap, HashSet};
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct SkillChain {
    pub name: String,
    pub steps: Vec<ChainStep>,
    pub on_failure: FailureStrategy,
}

#[derive(Debug, Clone)]
pub struct ChainStep {
    pub skill_name: String,
    pub input_mapping: HashMap<String, String>,
    pub output_key: Option<String>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone)]
pub enum FailureStrategy {
    Stop,
    Skip,
    Retry(u32),
    Fallback(String),
}

pub struct ChainOrchestrator {
    chains: HashMap<String, SkillChain>,
    execution_history: Vec<ExecutionRecord>,
}

#[derive(Debug, Clone)]
struct ExecutionRecord {
    chain_name: String,
    step_index: usize,
    status: TaskStatus,
    duration_ms: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl ChainOrchestrator {
    pub fn new() -> Self {
        Self {
            chains: HashMap::new(),
            execution_history: Vec::new(),
        }
    }
    
    pub fn register_chain(&mut self, chain: SkillChain) {
        self.chains.insert(chain.name.clone(), chain);
    }
    
    pub fn build_chain(&self, skills: &[SkillMetadata]) -> SkillChain {
        let steps: Vec<ChainStep> = skills
            .iter()
            .enumerate()
            .map(|(idx, skill)| ChainStep {
                skill_name: skill.name.clone(),
                input_mapping: if idx > 0 {
                    let mut map = HashMap::new();
                    map.insert("input".to_string(), format!("step_{}.output", idx - 1));
                    map
                } else {
                    HashMap::new()
                },
                output_key: Some(format!("step_{}.output", idx)),
                condition: None,
            })
            .collect();
        
        SkillChain {
            name: format!("chain_{}", chrono::Utc::now().timestamp()),
            steps,
            on_failure: FailureStrategy::Stop,
        }
    }
    
    pub fn auto_chain(
        &self,
        registry: &crate::models::Registry,
        required_caps: &[String],
    ) -> Option<SkillChain> {
        let mut ordered_skills: Vec<SkillMetadata> = Vec::new();
        let mut covered_caps: HashSet<String> = HashSet::new();
        let mut remaining_caps: Vec<String> = required_caps.to_vec();
        
        let mut iterations = 0;
        while !remaining_caps.is_empty() && iterations < 10 {
            iterations += 1;
            
            if let Some(skill) = self.find_best_skill_for_remaining(
                registry,
                &remaining_caps,
                &covered_caps,
            ) {
                for cap in &skill.capabilities {
                    covered_caps.insert(cap.clone());
                }
                remaining_caps.retain(|c| !covered_caps.contains(c));
                ordered_skills.push(skill);
            } else {
                break;
            }
        }
        
        if ordered_skills.is_empty() {
            return None;
        }
        
        Some(self.build_chain(&ordered_skills))
    }
    
    fn find_best_skill_for_remaining(
        &self,
        registry: &crate::models::Registry,
        remaining_caps: &[String],
        covered_caps: &HashSet<String>,
    ) -> Option<SkillMetadata> {
        let mut best: Option<(SkillMetadata, usize)> = None;
        
        for skill in registry.skills.values() {
            let new_caps_count = skill.capabilities.iter()
                .filter(|c| remaining_caps.contains(c) && !covered_caps.contains(*c))
                .count();
            
            if new_caps_count > 0 {
                if let Some((_, count)) = &best {
                    if new_caps_count > *count {
                        best = Some((skill.clone(), new_caps_count));
                    }
                } else {
                    best = Some((skill.clone(), new_caps_count));
                }
            }
        }
        
        best.map(|(s, _)| s)
    }
    
    pub async fn execute_chain(
        &mut self,
        chain: &SkillChain,
        initial_input: Option<&str>,
        registry: &crate::models::Registry,
    ) -> Result<ExecutionResult> {
        let task_id = format!("chain_{}", chrono::Utc::now().timestamp_millis());
        let mut context: HashMap<String, serde_json::Value> = HashMap::new();
        
        if let Some(input) = initial_input {
            context.insert("input".to_string(), serde_json::json!(input));
        }
        
        let mut results = Vec::new();
        let start = std::time::Instant::now();
        
        for (idx, step) in chain.steps.iter().enumerate() {
            let step_start = std::time::Instant::now();
            
            if let Some(condition) = &step.condition {
                if !self.evaluate_condition(condition, &context) {
                    results.push(SubTaskResult {
                        subtask_id: format!("step_{}", idx),
                        status: TaskStatus::Skipped,
                        output: Some("Condition not met".to_string()),
                        error: None,
                        duration_ms: 0,
                    });
                    continue;
                }
            }
            
            let skill = registry.skills.get(&step.skill_name)
                .ok_or_else(|| anyhow!("Skill '{}' not found", step.skill_name))?;
            
            let result = self.execute_step(skill, &context).await;
            
            let record = ExecutionRecord {
                chain_name: chain.name.clone(),
                step_index: idx,
                status: result.status.clone(),
                duration_ms: step_start.elapsed().as_millis() as u64,
                timestamp: chrono::Utc::now(),
            };
            self.execution_history.push(record);
            
            if result.status == TaskStatus::Failed {
                match &chain.on_failure {
                    FailureStrategy::Stop => {
                        results.push(result);
                        break;
                    }
                    FailureStrategy::Skip => {
                        results.push(SubTaskResult {
                            subtask_id: format!("step_{}", idx),
                            status: TaskStatus::Skipped,
                            output: None,
                            error: Some("Skipped due to previous failure".to_string()),
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        });
                        continue;
                    }
                    FailureStrategy::Fallback(fallback_name) => {
                        if let Some(fallback) = registry.skills.get(fallback_name) {
                            let fallback_result = self.execute_step(fallback, &context).await;
                            results.push(fallback_result);
                        }
                        continue;
                    }
                    FailureStrategy::Retry(n) => {
                        let mut retry_result = result.clone();
                        for _ in 0..*n {
                            let retried = self.execute_step(skill, &context).await;
                            if retried.status == TaskStatus::Completed {
                                retry_result = retried;
                                break;
                            }
                        }
                        results.push(retry_result);
                        continue;
                    }
                }
            } else {
                if let Some(output_key) = &step.output_key {
                    if let Some(output) = &result.output {
                        context.insert(output_key.clone(), serde_json::json!(output));
                    }
                }
                results.push(result);
            }
        }
        
        let all_completed = results.iter().all(|r| r.status == TaskStatus::Completed || r.status == TaskStatus::Skipped);
        
        Ok(ExecutionResult {
            task_id,
            status: if all_completed { TaskStatus::Completed } else { TaskStatus::Failed },
            output: context.get("output").map(|v| v.to_string()),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            skill_used: Some(chain.name.clone()),
            subtask_results: results,
        })
    }
    
    async fn execute_step(
        &self,
        skill: &SkillMetadata,
        context: &HashMap<String, serde_json::Value>,
    ) -> SubTaskResult {
        let start = std::time::Instant::now();
        
        let config = crate::models::Config {
            enable_auto_install: false,
            skills_dir: "skills".to_string(),
            registry_file: "registry.json".to_string(),
            logs_dir: "logs".to_string(),
            trusted_sources: vec![],
            llm_enabled: None,
            llm_command: None,
            llm_endpoint: None,
            max_retries: None,
            parallel_workers: None,
            cache_ttl_seconds: None,
        };
        
        match crate::executor::Executor::execute(&config, skill, true) {
            Ok(_) => SubTaskResult {
                subtask_id: skill.name.clone(),
                status: TaskStatus::Completed,
                output: Some(format!("Skill {} completed", skill.name)),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
            Err(e) => SubTaskResult {
                subtask_id: skill.name.clone(),
                status: TaskStatus::Failed,
                output: None,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
    
    fn evaluate_condition(&self, condition: &str, context: &HashMap<String, serde_json::Value>) -> bool {
        if condition == "always" {
            return true;
        }
        if condition == "never" {
            return false;
        }
        
        if let Some(value) = context.get(condition) {
            return !value.is_null();
        }
        
        true
    }
    
    pub fn get_execution_history(&self) -> &[ExecutionRecord] {
        &self.execution_history
    }
}