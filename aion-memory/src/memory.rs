use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ── Data Structures ──────────────────────────────────────────────────────────

/// A single memory entry representing a fact, decision, or lesson learned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub category: MemoryCategory,
    pub content: String,
    #[serde(default)]
    pub source_session: String,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default = "default_importance")]
    pub importance: u8, // 1-10
    #[serde(default)]
    pub access_count: u64,
    #[serde(default)]
    pub last_accessed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryCategory {
    Decision,
    Lesson,
    Error,
    Preference,
    Architecture,
    TaskProgress,
}

/// The persistent memory store, serialized as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStore {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub entries: Vec<MemoryEntry>,
    #[serde(default)]
    pub last_updated: u64,
}

fn default_importance() -> u8 {
    5
}

fn default_version() -> String {
    "1.0.0".to_string()
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            entries: Vec::new(),
            last_updated: now_epoch(),
        }
    }
}

// ── Core Memory Manager ──────────────────────────────────────────────────────

pub struct MemoryManager {
    store_path: PathBuf,
    context_path: PathBuf,
    /// 内存缓存层：用 RwLock 保护，recall 走读锁（零磁盘 IO），remember 走写锁
    cache: std::sync::RwLock<Option<MemoryStore>>,
}

impl MemoryManager {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            store_path: workspace_root.join("memory_store.json"),
            context_path: workspace_root.join("CONTEXT.md"),
            cache: std::sync::RwLock::new(None),
        }
    }

    /// 获取缓存的 store（命中缓存则零 IO）
    fn load_cached(&self) -> Result<MemoryStore> {
        // 快路径：读锁检查缓存
        if let Ok(guard) = self.cache.read() {
            if let Some(ref store) = *guard {
                return Ok(store.clone());
            }
        }
        // 慢路径：从磁盘加载并填充缓存
        let store = self.load_from_disk()?;
        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(store.clone());
        }
        Ok(store)
    }

    /// 保存到磁盘并更新缓存
    fn save_and_cache(&self, store: &MemoryStore) -> Result<()> {
        self.save_to_disk(store)?;
        if let Ok(mut guard) = self.cache.write() {
            *guard = Some(store.clone());
        }
        Ok(())
    }

    // ── Load / Save ──────────────────────────────────────────────────────

    /// 公开的 load 接口（走缓存）
    pub fn load(&self) -> Result<MemoryStore> {
        self.load_cached()
    }

    /// 公开的 save 接口（同时更新缓存）
    pub fn save(&self, store: &MemoryStore) -> Result<()> {
        self.save_and_cache(store)
    }

    fn load_from_disk(&self) -> Result<MemoryStore> {
        if self.store_path.exists() {
            let data = fs::read_to_string(&self.store_path)?;
            let store: MemoryStore = serde_json::from_str(&data)?;
            Ok(store)
        } else {
            Ok(MemoryStore::new())
        }
    }

    fn save_to_disk(&self, store: &MemoryStore) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(store)?;
        fs::write(&self.store_path, json)?;
        Ok(())
    }

    // ── Remember ─────────────────────────────────────────────────────────

    /// Auto-distillation threshold: when entries exceed this count after a write,
    /// distillation is triggered automatically.
    const AUTO_DISTILL_THRESHOLD: usize = 150;
    /// Target entry count after auto-distillation.
    const AUTO_DISTILL_TARGET: usize = 120;

    pub fn remember(
        &self,
        category: MemoryCategory,
        content: &str,
        session_id: &str,
        importance: u8,
    ) -> Result<String> {
        let mut store = self.load()?;
        let id = format!("mem_{}", now_epoch());
        let entry = MemoryEntry {
            id: id.clone(),
            category,
            content: content.to_string(),
            source_session: session_id.to_string(),
            timestamp: now_epoch(),
            importance: importance.clamp(1, 10),
            access_count: 0,
            last_accessed: 0,
        };
        store.entries.push(entry);
        store.last_updated = now_epoch();
        self.save(&store)?;

        // Auto-distill when memory exceeds threshold
        if store.entries.len() > Self::AUTO_DISTILL_THRESHOLD {
            tracing::info!(
                "Memory store has {} entries (threshold {}), triggering auto-distillation",
                store.entries.len(),
                Self::AUTO_DISTILL_THRESHOLD
            );
            match crate::memory_distiller::MemoryDistiller::distill(self, Self::AUTO_DISTILL_TARGET) {
                Ok(report) => {
                    tracing::info!(
                        "Auto-distillation complete: {} → {} entries (dedup: {}, evicted: {}, merged: {})",
                        report.original_count,
                        report.final_count,
                        report.duplicates_removed,
                        report.low_value_evicted,
                        report.lessons_merged
                    );
                }
                Err(e) => {
                    tracing::warn!("Auto-distillation failed (non-fatal): {}", e);
                }
            }
        }

        Ok(id)
    }

    // ── Recall (Keyword Search) ──────────────────────────────────────────

    pub fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let mut store = self.load()?;
        let query_lower = query.to_ascii_lowercase();
        let keywords: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(usize, usize)> = store
            .entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let content_lower = entry.content.to_ascii_lowercase();
                let keyword_hits = keywords
                    .iter()
                    .filter(|kw| content_lower.contains(*kw))
                    .count();
                let importance_bonus = entry.importance as usize;
                (idx, keyword_hits * 10 + importance_bonus)
            })
            .filter(|(_, score)| *score > 0)
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(limit);

        // Update access counts
        for (idx, _) in &scored {
            store.entries[*idx].access_count += 1;
            store.entries[*idx].last_accessed = now_epoch();
        }
        self.save(&store)?;

        Ok(scored
            .iter()
            .map(|(idx, _)| store.entries[*idx].clone())
            .collect())
    }

    // ── Recall by Category ───────────────────────────────────────────────

    pub fn recall_by_category(
        &self,
        category: &MemoryCategory,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let store = self.load()?;
        let mut matched: Vec<MemoryEntry> = store
            .entries
            .into_iter()
            .filter(|e| e.category == *category)
            .collect();
        matched.sort_by(|a, b| b.importance.cmp(&a.importance));
        matched.truncate(limit);
        Ok(matched)
    }

    // ── Generate CONTEXT.md ──────────────────────────────────────────────

    pub fn generate_context_md(&self) -> Result<String> {
        let store = self.load()?;
        let mut md = String::from("# Project Context (Auto-Generated)\n\n");
        md.push_str(&format!(
            "> Last updated: {}\n\n",
            now_epoch()
        ));

        let categories = [
            (MemoryCategory::Architecture, "Architecture Decisions"),
            (MemoryCategory::TaskProgress, "Task Progress"),
            (MemoryCategory::Decision, "Key Decisions"),
            (MemoryCategory::Lesson, "Lessons Learned"),
            (MemoryCategory::Error, "Known Error Patterns"),
            (MemoryCategory::Preference, "User Preferences"),
        ];

        for (cat, title) in &categories {
            let entries: Vec<&MemoryEntry> = store
                .entries
                .iter()
                .filter(|e| e.category == *cat)
                .collect();
            if entries.is_empty() {
                continue;
            }
            md.push_str(&format!("## {}\n\n", title));
            for entry in entries {
                md.push_str(&format!("- **[{}]** {}\n", entry.id, entry.content));
            }
            md.push('\n');
        }

        // Persist to file
        fs::write(&self.context_path, &md)?;
        Ok(md)
    }

    // ── Statistics ────────────────────────────────────────────────────────

    pub fn stats(&self) -> Result<Value> {
        let store = self.load()?;
        let total = store.entries.len();
        let by_category = |cat: &MemoryCategory| {
            store.entries.iter().filter(|e| e.category == *cat).count()
        };
        Ok(json!({
            "total_memories": total,
            "decisions": by_category(&MemoryCategory::Decision),
            "lessons": by_category(&MemoryCategory::Lesson),
            "errors": by_category(&MemoryCategory::Error),
            "preferences": by_category(&MemoryCategory::Preference),
            "architecture": by_category(&MemoryCategory::Architecture),
            "task_progress": by_category(&MemoryCategory::TaskProgress),
            "store_version": store.version,
        }))
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
