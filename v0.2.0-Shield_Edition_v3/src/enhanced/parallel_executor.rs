use crate::models::{TaskPlan, SubTask, TaskStatus, ExecutionResult, SubTaskResult, Config, SkillMetadata};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use anyhow::Result;

pub struct ParallelExecutor {
    config: Config,
    max_workers: usize,
}

impl ParallelExecutor {
    pub fn new(config: Config) -> Self {
        let workers = config.parallel_workers.unwrap_or(4);
        Self {
            config,
            max_workers: workers,
        }
    }
    
    pub async fn execute_plan(
        &self,
        plan: TaskPlan,
        registry: Arc<RwLock<crate::models::Registry>>,
    ) -> ExecutionResult {
        let start = Instant::now();
        let mut results = Vec::new();
        
        match plan.execution_strategy {
            crate::models::ExecutionStrategy::Sequential => {
                for subtask in &plan.subtasks {
                    let result = self.execute_subtask(subtask, registry.clone()).await;
                    results.push(result);
                }
            }
            crate::models::ExecutionStrategy::Parallel => {
                let mut handles = Vec::new();
                for subtask in &plan.subtasks {
                    let st = subtask.clone();
                    let reg = registry.clone();
                    let handle = tokio::spawn(async move {
                        Self::execute_subtask_static(&st, reg).await
                    });
                    handles.push(handle);
                }
                
                for handle in handles {
                    if let Ok(result) = handle.await {
                        results.push(result);
                    }
                }
            }
            crate::models::ExecutionStrategy::Pipeline => {
                results = self.execute_pipeline(&plan.subtasks, registry.clone()).await;
            }
            crate::models::ExecutionStrategy::Adaptive => {
                results = self.execute_adaptive(&plan.subtasks, registry.clone()).await;
            }
        }
        
        let failed_count = results.iter().filter(|r| r.status == TaskStatus::Failed).count();
        let overall_status = if failed_count == 0 {
            TaskStatus::Completed
        } else if failed_count == results.len() {
            TaskStatus::Failed
        } else {
            TaskStatus::Completed
        };
        
        ExecutionResult {
            task_id: plan.task_id,
            status: overall_status,
            output: Some(format!("Processed {} subtasks", results.len())),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            skill_used: None,
            subtask_results: results,
        }
    }
    
    async fn execute_subtask(
        &self,
        subtask: &SubTask,
        registry: Arc<RwLock<crate::models::Registry>>,
    ) -> SubTaskResult {
        Self::execute_subtask_static(subtask, registry).await
    }
    
    async fn execute_subtask_static(
        subtask: &SubTask,
        registry: Arc<RwLock<crate::models::Registry>>,
    ) -> SubTaskResult {
        let start = Instant::now();
        
        let skill_opt = {
            let reg = registry.read().await;
            crate::matcher::Matcher::find_best_match(&reg, &subtask.required_capabilities)
        };
        
        if let Some(skill) = skill_opt {
            let config = Config {
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
            
            match crate::executor::Executor::execute(&config, &skill, true) {
                Ok(_) => SubTaskResult {
                    subtask_id: subtask.id.clone(),
                    status: TaskStatus::Completed,
                    output: Some(format!("Skill {} executed", skill.name)),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                },
                Err(e) => SubTaskResult {
                    subtask_id: subtask.id.clone(),
                    status: TaskStatus::Failed,
                    output: None,
                    error: Some(e.to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            }
        } else {
            SubTaskResult {
                subtask_id: subtask.id.clone(),
                status: TaskStatus::Failed,
                output: None,
                error: Some("No matching skill found".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
    }
    
    async fn execute_pipeline(
        &self,
        subtasks: &[SubTask],
        registry: Arc<RwLock<crate::models::Registry>>,
    ) -> Vec<SubTaskResult> {
        let mut results = Vec::new();
        let mut completed_ids = std::collections::HashSet::new();
        let mut pending: Vec<&SubTask> = subtasks.iter().collect();
        
        while !pending.is_empty() {
            let ready: Vec<&SubTask> = pending.iter()
                .filter(|s| s.dependencies.iter().all(|d| completed_ids.contains(d)))
                .copied()
                .collect();
            
            if ready.is_empty() && !pending.is_empty() {
                let remaining = pending.remove(0);
                results.push(SubTaskResult {
                    subtask_id: remaining.id.clone(),
                    status: TaskStatus::Skipped,
                    output: None,
                    error: Some("Unresolvable dependencies".to_string()),
                    duration_ms: 0,
                });
                continue;
            }
            
            for subtask in ready {
                let result = self.execute_subtask(subtask, registry.clone()).await;
                if result.status == TaskStatus::Completed {
                    completed_ids.insert(subtask.id.clone());
                }
                results.push(result);
                pending.retain(|s| s.id != subtask.id);
            }
        }
        
        results
    }
    
    async fn execute_adaptive(
        &self,
        subtasks: &[SubTask],
        registry: Arc<RwLock<crate::models::Registry>>,
    ) -> Vec<SubTaskResult> {
        let has_deps = subtasks.iter().any(|s| !s.dependencies.is_empty());
        
        if has_deps {
            self.execute_pipeline(subtasks, registry).await
        } else {
            let mut handles = Vec::new();
            for subtask in subtasks {
                let st = subtask.clone();
                let reg = registry.clone();
                let handle = tokio::spawn(async move {
                    Self::execute_subtask_static(&st, reg).await
                });
                handles.push(handle);
            }
            
            let mut results = Vec::new();
            for handle in handles {
                if let Ok(result) = handle.await {
                    results.push(result);
                }
            }
            results
        }
    }
}