//! 命名空间记忆管理器
//!
//! 为多 Agent 场景提供三级记忆隔离：
//!
//! - **Global** — 所有 Agent 共享的只读知识库
//! - **Team** — 同一 session 中协作 Agent 的共享记忆
//! - **Private** — 单个 Agent 的私有记忆
//!
//! ```text
//! memory/
//! ├── global/memory_store.json        ← Global（所有 Agent 只读）
//! ├── team/{session_id}/memory_store.json  ← Team（同 session 读写）
//! └── agents/{agent_id}/memory_store.json  ← Private（Agent 私有）
//! ```

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::memory::{MemoryCategory, MemoryEntry, MemoryManager};

// ── 命名空间定义 ────────────────────────────────────────────────────────────

/// 记忆命名空间
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryNamespace {
    /// 全局共享记忆（只读，管理员可写）
    Global,
    /// 团队记忆（同 session 的多 Agent 共享读写）
    Team { session_id: String },
    /// 私有记忆（单个 Agent 独占）
    Private { agent_id: String },
}

impl MemoryNamespace {
    /// 返回该命名空间在磁盘上的子目录路径
    fn subdirectory(&self) -> PathBuf {
        match self {
            Self::Global => PathBuf::from("memory").join("global"),
            Self::Team { session_id } => PathBuf::from("memory").join("team").join(session_id),
            Self::Private { agent_id } => PathBuf::from("memory").join("agents").join(agent_id),
        }
    }
}

/// 记忆可见性范围
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryScope {
    /// 仅记忆所属 Agent 可见
    Private,
    /// 同 session 的所有 Agent 可见
    Team,
    /// 所有 Agent 可见（需要管理员权限写入）
    Public,
}

// ── 命名空间记忆管理器 ──────────────────────────────────────────────────────

/// 支持多命名空间的记忆管理器
///
/// 包装底层 `MemoryManager`，为每个命名空间创建独立的存储路径。
pub struct NamespacedMemoryManager {
    /// 工作区根目录
    workspace_root: PathBuf,
}

impl NamespacedMemoryManager {
    /// 创建命名空间记忆管理器
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    /// 获取指定命名空间的底层 MemoryManager
    pub fn for_namespace(&self, ns: &MemoryNamespace) -> MemoryManager {
        let ns_dir = self.workspace_root.join(ns.subdirectory());
        MemoryManager::new(&ns_dir)
    }

    /// 在指定命名空间中存储记忆
    pub fn remember(
        &self,
        ns: &MemoryNamespace,
        category: MemoryCategory,
        content: &str,
        session_id: &str,
        importance: u8,
    ) -> Result<String> {
        self.for_namespace(ns).remember(category, content, session_id, importance)
    }

    /// 在指定命名空间中召回记忆
    pub fn recall(
        &self,
        ns: &MemoryNamespace,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        self.for_namespace(ns).recall(query, limit)
    }

    /// 跨命名空间召回（按优先级：Private → Team → Global）
    pub fn recall_cascading(
        &self,
        agent_id: &str,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>> {
        let mut all_entries = Vec::new();

        // 1. 私有记忆（最高优先级）
        let private_ns = MemoryNamespace::Private { agent_id: agent_id.to_string() };
        if let Ok(entries) = self.recall(&private_ns, query, limit) {
            all_entries.extend(entries);
        }

        // 2. 团队记忆
        let team_ns = MemoryNamespace::Team { session_id: session_id.to_string() };
        if let Ok(entries) = self.recall(&team_ns, query, limit) {
            all_entries.extend(entries);
        }

        // 3. 全局记忆（最低优先级）
        if let Ok(entries) = self.recall(&MemoryNamespace::Global, query, limit) {
            all_entries.extend(entries);
        }

        // 按重要度+访问次数排序，去重
        all_entries.sort_by(|a, b| {
            let score_a = a.importance as u64 * 10 + a.access_count;
            let score_b = b.importance as u64 * 10 + b.access_count;
            score_b.cmp(&score_a)
        });

        // 按内容去重
        let mut seen = std::collections::HashSet::new();
        all_entries.retain(|e| seen.insert(e.content.clone()));

        all_entries.truncate(limit);
        Ok(all_entries)
    }

    /// 将私有记忆提升为团队共享
    pub fn promote_to_team(
        &self,
        agent_id: &str,
        session_id: &str,
        memory_id: &str,
    ) -> Result<()> {
        let private_ns = MemoryNamespace::Private { agent_id: agent_id.to_string() };
        let team_ns = MemoryNamespace::Team { session_id: session_id.to_string() };

        let private_mgr = self.for_namespace(&private_ns);
        let store = private_mgr.load()?;

        let entry = store.entries.iter()
            .find(|e| e.id == memory_id)
            .ok_or_else(|| anyhow::anyhow!("memory entry '{}' not found in agent '{}'", memory_id, agent_id))?;

        let team_mgr = self.for_namespace(&team_ns);
        team_mgr.remember(
            entry.category.clone(),
            &entry.content,
            &entry.source_session,
            entry.importance,
        )?;

        Ok(())
    }

    /// 获取命名空间的统计信息
    pub fn stats(&self, ns: &MemoryNamespace) -> Result<serde_json::Value> {
        self.for_namespace(ns).stats()
    }

    /// 列出所有已知的命名空间（扫描磁盘目录）
    pub fn list_namespaces(&self) -> Vec<MemoryNamespace> {
        let mut namespaces = Vec::new();

        // Global
        let global_path = self.workspace_root.join("memory").join("global").join("memory_store.json");
        if global_path.exists() {
            namespaces.push(MemoryNamespace::Global);
        }

        // Team
        let team_dir = self.workspace_root.join("memory").join("team");
        if let Ok(entries) = std::fs::read_dir(&team_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Some(name) = entry.file_name().to_str() {
                        namespaces.push(MemoryNamespace::Team { session_id: name.to_string() });
                    }
                }
            }
        }

        // Private
        let agents_dir = self.workspace_root.join("memory").join("agents");
        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Some(name) = entry.file_name().to_str() {
                        namespaces.push(MemoryNamespace::Private { agent_id: name.to_string() });
                    }
                }
            }
        }

        namespaces
    }
}
