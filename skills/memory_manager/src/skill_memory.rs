use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 技能使用记录
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillUsageRecord {
    pub skill_id: String,
    pub timestamp: String,
    pub input: String,
    pub output: String,
    pub success: bool,
    pub execution_time_ms: u64,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl SkillUsageRecord {
    pub fn new(
        skill_id: &str,
        input: &str,
        output: &str,
        success: bool,
        execution_time_ms: u64,
        tags: Vec<String>,
    ) -> Self {
        SkillUsageRecord {
            skill_id: skill_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            input: input.to_string(),
            output: output.to_string(),
            success,
            execution_time_ms,
            tags,
            metadata: HashMap::new(),
        }
    }
}

/// 技能记忆存储
#[derive(Debug, Serialize, Deserialize)]
pub struct SkillMemoryStore {
    pub records: Vec<SkillUsageRecord>,
    pub skill_stats: HashMap<String, SkillStats>,
    pub last_updated: String,
}

impl SkillMemoryStore {
    pub fn new() -> Self {
        SkillMemoryStore {
            records: Vec::new(),
            skill_stats: HashMap::new(),
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn add_record(&mut self, record: SkillUsageRecord) {
        // 更新技能统计
        let skill_id = record.skill_id.clone();
        if let Some(stats) = self.skill_stats.get_mut(&skill_id) {
            stats.total_calls += 1;
            if record.success {
                stats.success_calls += 1;
            }
            stats.avg_latency_ms = (stats.avg_latency_ms * (stats.total_calls - 1) as f64
                + record.execution_time_ms as f64)
                / stats.total_calls as f64;
        } else {
            self.skill_stats.insert(
                skill_id.clone(),
                SkillStats {
                    total_calls: 1,
                    success_calls: if record.success { 1 } else { 0 },
                    avg_latency_ms: record.execution_time_ms as f64,
                },
            );
        }

        self.records.push(record);
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }

    pub fn get_skill_stats(&self, skill_id: &str) -> Option<&SkillStats> {
        self.skill_stats.get(skill_id)
    }

    pub fn search_by_skill(&self, skill_id: &str) -> Vec<&SkillUsageRecord> {
        self.records
            .iter()
            .filter(|r| r.skill_id == skill_id)
            .collect()
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&SkillUsageRecord> {
        self.records
            .iter()
            .filter(|r| r.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn search_by_keyword(&self, keyword: &str) -> Vec<&SkillUsageRecord> {
        self.records
            .iter()
            .filter(|r| r.input.contains(keyword) || r.output.contains(keyword))
            .collect()
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content).context("Failed to write skill memory store")
    }

    pub fn load(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content).context("Failed to parse skill memory store")
        } else {
            Ok(SkillMemoryStore::new())
        }
    }
}

/// 技能统计信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillStats {
    pub total_calls: usize,
    pub success_calls: usize,
    pub avg_latency_ms: f64,
}