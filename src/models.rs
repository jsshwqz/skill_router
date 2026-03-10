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
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Usage {
    pub total_calls: u64,
    pub success_calls: u64,
    pub failed_calls: u64,
    pub avg_latency_ms: f64,
    pub last_used: String, // ISO 8601 string
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lifecycle {
    pub decision: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Registry {
    pub skills: HashMap<String, SkillMetadata>,
}
