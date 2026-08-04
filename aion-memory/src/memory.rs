use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
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
///
/// 升级到 redb 后仍保留该结构作为内存态表示与旧 JSON 迁移格式，
/// 序列化布局保持不变（#[serde(default)] 兼容）。
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

// ── redb 表结构 ──────────────────────────────────────────────────────────────

/// 记忆条目表：key = MemoryEntry.id，value = serde_json 序列化的 MemoryEntry
const ENTRIES_TABLE: TableDefinition<&str, &str> = TableDefinition::new("memory_entries");
/// 元信息表：version / last_updated
const META_TABLE: TableDefinition<&str, &str> = TableDefinition::new("memory_meta");
const META_KEY_VERSION: &str = "version";
const META_KEY_LAST_UPDATED: &str = "last_updated";

/// 进程级 redb 实例注册表：同一 db 路径共享同一个 `Database` 实例，
/// 避免同进程对同一文件重复打开触发 redb 文件锁冲突
/// （`namespaced_memory::for_namespace` 等调用方会为同一路径反复创建 `MemoryManager`）。
fn db_registry() -> &'static std::sync::Mutex<HashMap<PathBuf, std::sync::Arc<Database>>> {
    static REGISTRY: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, std::sync::Arc<Database>>>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

// ── Core Memory Manager ──────────────────────────────────────────────────────

pub struct MemoryManager {
    db_path: PathBuf,
    context_path: PathBuf,
    /// 旧版 JSON 存储路径（用于一次性迁移，导入后保留原文件作为备份）
    legacy_json_path: PathBuf,
}

impl MemoryManager {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            db_path: workspace_root.join("memory_store.redb"),
            context_path: workspace_root.join("CONTEXT.md"),
            legacy_json_path: workspace_root.join("memory_store.json"),
        }
    }

    /// 获取（或打开）该 manager 对应的 redb 数据库（进程内按路径共享实例）。
    /// redb 自带事务与 MVCC，替换原 std::sync::RwLock 缓存层。
    fn db(&self) -> Result<std::sync::Arc<Database>> {
        let path = self.db_path.clone();
        let mut registry = db_registry().lock().expect("redb registry lock poisoned");
        if let Some(db) = registry.get(&path) {
            return Ok(db.clone());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // redb 4.1 的 Database::create 兼容两种场景：文件不存在则初始化新库，已是有效 redb 文件则打开。
        let db = std::sync::Arc::new(Database::create(&path)?);
        self.migrate_if_needed(&db)?;
        Ok(registry.entry(path).or_insert(db).clone())
    }

    /// 一次性迁移：若旧 `memory_store.json` 存在且 redb 条目表为空，则把旧数据导入 redb。
    /// 导入后保留旧 JSON 文件作为备份（不删除，避免丢记忆）。
    fn migrate_if_needed(&self, db: &Database) -> Result<()> {
        if !self.legacy_json_path.exists() {
            return Ok(());
        }
        {
            let read_tx = db.begin_read()?;
            if let Ok(table) = read_tx.open_table(ENTRIES_TABLE) {
                if !table.is_empty()? {
                    return Ok(()); // redb 已有数据（此前已迁移），跳过
                }
            }
        }
        tracing::info!(
            "Migrating legacy memory_store.json into redb ({:?})",
            self.legacy_json_path
        );
        let data = std::fs::read_to_string(&self.legacy_json_path)
            .with_context(|| format!("failed to read legacy memory store {:?}", self.legacy_json_path))?;
        let store: MemoryStore = serde_json::from_str(&data)
            .with_context(|| format!("failed to parse legacy memory store {:?}", self.legacy_json_path))?;
        let write_tx = db.begin_write()?;
        {
            let mut table = write_tx.open_table(ENTRIES_TABLE)?;
            for entry in &store.entries {
                let json = serde_json::to_string(entry)?;
                table.insert(entry.id.as_str(), json.as_str())?;
            }
            let mut meta = write_tx.open_table(META_TABLE)?;
            meta.insert(META_KEY_VERSION, store.version.as_str())?;
            meta.insert(META_KEY_LAST_UPDATED, store.last_updated.to_string().as_str())?;
        }
        write_tx.commit()?;
        tracing::info!(
            "Migrated {} entries from memory_store.json to redb",
            store.entries.len()
        );
        Ok(())
    }

    /// 从 redb 读取全部记忆（直接读库，redb 自带页缓存与并发控制）
    fn read_all(&self) -> Result<MemoryStore> {
        let db = self.db()?;
        let read_tx = db.begin_read()?;
        let mut store = MemoryStore::new();
        if let Ok(table) = read_tx.open_table(ENTRIES_TABLE) {
            for pair in table.iter()? {
                let (_, value) = pair?;
                let entry: MemoryEntry = serde_json::from_str(value.value())
                    .with_context(|| "corrupt memory entry in redb store".to_string())?;
                store.entries.push(entry);
            }
        }
        if let Ok(meta) = read_tx.open_table(META_TABLE) {
            if let Some(v) = meta.get(META_KEY_VERSION)? {
                store.version = v.value().to_string();
            }
            if let Some(ts) = meta.get(META_KEY_LAST_UPDATED)? {
                if let Ok(parsed) = ts.value().parse::<u64>() {
                    store.last_updated = parsed;
                }
            }
        }
        Ok(store)
    }

    // ── Load / Save ──────────────────────────────────────────────────────

    /// 公开的 load 接口：从 redb 读取全量 store
    pub fn load(&self) -> Result<MemoryStore> {
        self.read_all()
    }

    /// 公开的 save 接口：以单个原子写事务覆盖写整个 store
    /// （清空后重写条目表，保证 distiller 等调用方删除/修改后与 redb 一致）
    pub fn save(&self, store: &MemoryStore) -> Result<()> {
        let db = self.db()?;
        let write_tx = db.begin_write()?;
        {
            let mut table = write_tx.open_table(ENTRIES_TABLE)?;
            table.retain(|_, _| false)?;
            for entry in &store.entries {
                let json = serde_json::to_string(entry)?;
                table.insert(entry.id.as_str(), json.as_str())?;
            }
            let mut meta = write_tx.open_table(META_TABLE)?;
            meta.insert(META_KEY_VERSION, store.version.as_str())?;
            meta.insert(META_KEY_LAST_UPDATED, store.last_updated.to_string().as_str())?;
        }
        write_tx.commit()?;
        Ok(())
    }

    /// 公开接口：直接从存储读取（redb 下与 `load` 等价，无用户缓存层）
    pub fn load_raw(&self) -> Result<MemoryStore> {
        self.read_all()
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
        let id = format!("mem_{}", uuid::Uuid::new_v4().simple());
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
        let mut store = self.load_raw()?; // 直接读库，无过期缓存问题
        let query_lower = query.to_ascii_lowercase();
        let keywords: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(usize, usize)> = store
            .entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let content_lower = entry.content.to_ascii_lowercase();
                let keyword_hits = keywords.iter().filter(|kw| content_lower.contains(*kw)).count();
                let importance_bonus = entry.importance as usize;
                (idx, keyword_hits * 10 + importance_bonus)
            })
            .filter(|(_, score)| *score > 0)
            .collect();

        scored.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        scored.truncate(limit);

        // Update access counts in-place
        for (idx, _) in &scored {
            store.entries[*idx].access_count += 1;
            store.entries[*idx].last_accessed = now_epoch();
        }
        store.last_updated = now_epoch();

        // Save with cache update — now it contains all the modified entries including recall's access count changes
        self.save(&store)?;

        Ok(scored.iter().map(|(idx, _)| store.entries[*idx].clone()).collect())
    }

    // ── Recall by Category ───────────────────────────────────────────────

    pub fn recall_by_category(&self, category: &MemoryCategory, limit: usize) -> Result<Vec<MemoryEntry>> {
        let store = self.load()?;
        let mut matched: Vec<MemoryEntry> = store.entries.into_iter().filter(|e| e.category == *category).collect();
        matched.sort_by_key(|entry| std::cmp::Reverse(entry.importance));
        matched.truncate(limit);
        Ok(matched)
    }

    // ── Generate CONTEXT.md ──────────────────────────────────────────────

    pub fn generate_context_md(&self) -> Result<String> {
        let store = self.load()?;
        let mut md = String::from("# Project Context (Auto-Generated)\n\n");
        md.push_str(&format!("> Last updated: {}\n\n", now_epoch()));

        let categories = [
            (MemoryCategory::Architecture, "Architecture Decisions"),
            (MemoryCategory::TaskProgress, "Task Progress"),
            (MemoryCategory::Decision, "Key Decisions"),
            (MemoryCategory::Lesson, "Lessons Learned"),
            (MemoryCategory::Error, "Known Error Patterns"),
            (MemoryCategory::Preference, "User Preferences"),
        ];

        for (cat, title) in &categories {
            let entries: Vec<&MemoryEntry> = store.entries.iter().filter(|e| e.category == *cat).collect();
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
        std::fs::write(&self.context_path, &md)?;
        Ok(md)
    }

    // ── Statistics ────────────────────────────────────────────────────────

    pub fn stats(&self) -> Result<Value> {
        let store = self.load()?;
        let total = store.entries.len();
        let by_category = |cat: &MemoryCategory| store.entries.iter().filter(|e| e.category == *cat).count();
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
