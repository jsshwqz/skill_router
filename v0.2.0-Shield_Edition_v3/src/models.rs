use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub enable_auto_install: bool,
    pub skills_dir: String,
    pub registry_file: String,
    pub logs_dir: String,
    pub trusted_sources: Vec<String>,
    pub llm_enabled: Option<bool>,
    pub llm_command: Option<String>,
    pub llm_endpoint: Option<String>,
    pub max_retries: Option<u32>,
    pub parallel_workers: Option<usize>,
    pub cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Usage {
    pub total_calls: u64,
    pub success_calls: u64,
    pub failed_calls: u64,
    pub avg_latency_ms: f64,
    pub last_used: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lifecycle {
    pub decision: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Permissions {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub filesystem_read: bool,
    #[serde(default)]
    pub filesystem_write: bool,
    #[serde(default)]
    pub process_exec: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub source: Option<String>,
    pub permissions: Permissions,
    pub usage: Option<Usage>,
    pub lifecycle: Option<Lifecycle>,
    pub description: Option<String>,
    pub entrypoint: Option<String>,
    pub tags: Option<Vec<String>>,
    pub dependencies: Option<Vec<String>>,
    pub priority: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Registry {
    pub skills: HashMap<String, SkillMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub task_id: String,
    pub original_task: String,
    pub subtasks: Vec<SubTask>,
    pub execution_strategy: ExecutionStrategy,
    pub estimated_complexity: ComplexityLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub description: String,
    pub required_capabilities: Vec<String>,
    pub dependencies: Vec<String>,
    pub status: TaskStatus,
    pub assigned_skill: Option<String>,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStrategy {
    Sequential,
    Parallel,
    Pipeline,
    Adaptive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplexityLevel {
    Simple,
    Medium,
    Complex,
    MultiStage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub skill_used: Option<String>,
    pub subtask_results: Vec<SubTaskResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTaskResult {
    pub subtask_id: String,
    pub status: TaskStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub total_tasks: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_task_duration_ms: f64,
    pub skills_executed: HashMap<String, SkillMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetrics {
    pub executions: u64,
    pub successes: u64,
    pub failures: u64,
    pub avg_latency_ms: f64,
    pub last_used: String,
}
