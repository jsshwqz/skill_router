//! 记忆类 builtin 技能：memory_remember, memory_recall, memory_distill, memory_team_share

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_memory::memory::{MemoryCategory, MemoryManager};
use aion_memory::memory_distiller::MemoryDistiller;
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

// ── memory_remember ─────────────────────────────────────────────────────────

pub struct MemoryRemember;

#[async_trait::async_trait]
impl BuiltinSkill for MemoryRemember {
    fn name(&self) -> &'static str { "memory_remember" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let content = context.context["content"]
            .as_str()
            .or_else(|| context.context["text"].as_str())
            .unwrap_or(&context.task)
            .to_string();
        let category_str = context.context["category"].as_str().unwrap_or("decision");
        let importance = context.context["importance"].as_u64().unwrap_or(5) as u8;
        let session = context.context["session"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let category = parse_category(category_str);
        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let id = manager.remember(category, &content, &session, importance)?;

        Ok(json!({
            "status": "remembered",
            "memory_id": id,
            "content": content,
        }))
    }
}

// ── memory_recall ───────────────────────────────────────────────────────────

pub struct MemoryRecall;

#[async_trait::async_trait]
impl BuiltinSkill for MemoryRecall {
    fn name(&self) -> &'static str { "memory_recall" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let query = context.context["query"]
            .as_str()
            .or_else(|| context.context["text"].as_str())
            .unwrap_or(&context.task)
            .to_string();
        let limit = context.context["limit"].as_u64().unwrap_or(5) as usize;

        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let results = manager.recall(&query, limit)?;

        let entries: Vec<Value> = results
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "content": e.content,
                    "category": format!("{:?}", e.category),
                    "importance": e.importance,
                    "access_count": e.access_count,
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results_count": entries.len(),
            "results": entries,
        }))
    }
}

// ── memory_distill ──────────────────────────────────────────────────────────

pub struct MemoryDistill;

#[async_trait::async_trait]
impl BuiltinSkill for MemoryDistill {
    fn name(&self) -> &'static str { "memory_distill" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let max_entries = context.context["max_entries"].as_u64().unwrap_or(200) as usize;

        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let report = MemoryDistiller::distill(&manager, max_entries)?;
        let _ = manager.generate_context_md();

        Ok(report.to_json())
    }
}

// ── memory_team_share ───────────────────────────────────────────────────────

pub struct MemoryTeamShare;

#[async_trait::async_trait]
impl BuiltinSkill for MemoryTeamShare {
    fn name(&self) -> &'static str { "memory_team_share" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let memory_id = context.context["memory_id"]
            .as_str()
            .ok_or_else(|| anyhow!("memory_team_share requires 'memory_id' in context"))?;
        let team_session = context.context["team_session_id"]
            .as_str()
            .unwrap_or("default");

        // 从个人记忆库读取
        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let entries = manager.recall(memory_id, 1)?;

        if entries.is_empty() {
            return Ok(json!({
                "memory_id": memory_id,
                "status": "not_found",
                "error": format!("记忆 '{}' 未找到", memory_id),
            }));
        }

        // 写入团队命名空间目录
        let team_dir = std::path::PathBuf::from(
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| ".".to_string())
        ).join(".aion").join("team").join(team_session);
        std::fs::create_dir_all(&team_dir)?;

        let entry = &entries[0];
        let team_file = team_dir.join(format!("{}.json", memory_id.replace(|c: char| !c.is_alphanumeric(), "_")));
        let team_entry = json!({
            "original_id": memory_id,
            "content": entry.content,
            "category": format!("{:?}", entry.category),
            "importance": entry.importance,
            "shared_by": "local",
            "shared_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        std::fs::write(&team_file, serde_json::to_string_pretty(&team_entry)?)?;

        Ok(json!({
            "memory_id": memory_id,
            "shared_to_team": team_session,
            "namespace": format!("team::{}", team_session),
            "team_file": team_file.to_string_lossy(),
            "status": "shared",
        }))
    }
}

// ── 工具函数 ────────────────────────────────────────────────────────────────

fn parse_category(s: &str) -> MemoryCategory {
    match s {
        "lesson" => MemoryCategory::Lesson,
        "error" => MemoryCategory::Error,
        "preference" => MemoryCategory::Preference,
        "architecture" => MemoryCategory::Architecture,
        "progress" => MemoryCategory::TaskProgress,
        _ => MemoryCategory::Decision,
    }
}
