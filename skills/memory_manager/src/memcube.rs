use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 立方体配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CubeConfig {
    pub embedding_model: String,
    pub vector_db: String,
    pub max_memories: usize,
    pub enable_search: bool,
}

impl Default for CubeConfig {
    fn default() -> Self {
        CubeConfig {
            embedding_model: "text-embedding-ada-002".to_string(),
            vector_db: "sqlite".to_string(),
            max_memories: 10000,
            enable_search: true,
        }
    }
}

/// 立方体元数据
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CubeMetadata {
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub description: String,
    pub is_shared: bool,
}

impl Default for CubeMetadata {
    fn default() -> Self {
        CubeMetadata {
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            tags: Vec::new(),
            description: "".to_string(),
            is_shared: false,
        }
    }
}

/// 记忆立方体
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryCube {
    pub cube_id: String,
    pub cube_name: String,
    pub config: CubeConfig,
    pub metadata: CubeMetadata,
    pub memories: Vec<MemoryEntry>,
    #[serde(default)]
    pub search_index: HashMap<String, usize>, // keyword -> memory index
}

impl MemoryCube {
    /// 创建新立方体
    pub fn new(cube_id: &str, cube_name: &str) -> Self {
        MemoryCube {
            cube_id: cube_id.to_string(),
            cube_name: cube_name.to_string(),
            config: CubeConfig::default(),
            metadata: CubeMetadata::default(),
            memories: Vec::new(),
            search_index: HashMap::new(),
        }
    }

    /// 添加记忆到立方体
    pub fn add_memory(&mut self, entry: MemoryEntry) {
        let keyword = self.extract_keyword(&entry.content);
        let index = self.memories.len();
        self.memories.push(entry);
        if !keyword.is_empty() {
            self.search_index.insert(keyword, index);
        }
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// 从立方体检索记忆
    pub fn retrieve(&self, keyword: &str, top_k: usize) -> Vec<&MemoryEntry> {
        // 简单实现：基于关键词匹配
        // 后续可集成向量搜索
        self.memories
            .iter()
            .filter(|e| e.content.contains(keyword))
            .take(top_k)
            .collect()
    }

    /// 提取关键词（简单实现）
    fn extract_keyword(&self, content: &str) -> String {
        content
            .split_whitespace()
            .filter(|s| s.len() > 3)
            .next()
            .unwrap_or("")
            .to_string()
    }

    /// 更新立方体配置
    pub fn update_config(&mut self, config: CubeConfig) {
        self.config = config;
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// 更新立方体元数据
    pub fn update_metadata(&mut self, metadata: CubeMetadata) {
        self.metadata = metadata;
    }

    /// 保存立方体到文件
    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content).context("Failed to write cube")
    }

    /// 从文件加载立方体
    pub fn load(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content).context("Failed to parse cube")
        } else {
            Ok(MemoryCube::new("default", "Default Cube"))
        }
    }
}

/// 记忆条目（与原 MemoryEntry 保持一致）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub timestamp: String,
    pub content: String,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl MemoryEntry {
    pub fn new(id: &str, content: &str, tags: Vec<String>) -> Self {
        MemoryEntry {
            id: id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            content: content.to_string(),
            tags,
            metadata: HashMap::new(),
        }
    }
}
