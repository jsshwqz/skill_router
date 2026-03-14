use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 上下文立方体配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextCubeConfig {
    pub max_files: usize,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub auto_update: bool,
    pub update_interval_hours: u32,
}

impl Default for ContextCubeConfig {
    fn default() -> Self {
        ContextCubeConfig {
            max_files: 100,
            include_patterns: vec!["*.rs".to_string(), "*.md".to_string(), "*.json".to_string()],
            exclude_patterns: vec!["target/".to_string(), ".git/".to_string()],
            auto_update: true,
            update_interval_hours: 24,
        }
    }
}

/// 上下文立方体元数据
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContextCubeMetadata {
    pub created_at: String,
    pub updated_at: String,
    pub last_scan: String,
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub tags: Vec<String>,
    pub description: String,
    pub is_shared: bool,
}

impl Default for ContextCubeMetadata {
    fn default() -> Self {
        ContextCubeMetadata {
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            last_scan: chrono::Utc::now().to_rfc3339(),
            file_count: 0,
            total_size_bytes: 0,
            tags: Vec::new(),
            description: "".to_string(),
            is_shared: false,
        }
    }
}

/// 上下文立方体 - 管理特定项目或模块的上下文
#[derive(Debug, Serialize, Deserialize)]
pub struct ContextCube {
    pub cube_id: String,
    pub cube_name: String,
    pub config: ContextCubeConfig,
    pub metadata: ContextCubeMetadata,
    pub files: HashMap<String, FileContext>,
    pub summaries: HashMap<String, String>, // file_path -> summary
}

impl ContextCube {
    pub fn new(cube_id: &str, cube_name: &str) -> Self {
        ContextCube {
            cube_id: cube_id.to_string(),
            cube_name: cube_name.to_string(),
            config: ContextCubeConfig::default(),
            metadata: ContextCubeMetadata::default(),
            files: HashMap::new(),
            summaries: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, file_path: &str, content: &str) {
        let file_context = FileContext {
            path: file_path.to_string(),
            content: content.to_string(),
            size: content.len() as u64,
            last_modified: chrono::Utc::now().to_rfc3339(),
            summary: generate_summary(content, 200),
        };
        
        self.files.insert(file_path.to_string(), file_context);
        self.metadata.file_count = self.files.len();
        self.metadata.total_size_bytes = self.files.values().map(|f| f.size).sum();
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn remove_file(&mut self, file_path: &str) {
        self.files.remove(file_path);
        self.summaries.remove(file_path);
        self.metadata.file_count = self.files.len();
        self.metadata.total_size_bytes = self.files.values().map(|f| f.size).sum();
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn get_file_content(&self, file_path: &str) -> Option<&String> {
        self.files.get(file_path).map(|f| &f.content)
    }

    pub fn get_file_summary(&self, file_path: &str) -> Option<&String> {
        if let Some(summary) = self.summaries.get(file_path) {
            Some(summary)
        } else if let Some(file) = self.files.get(file_path) {
            Some(&file.summary)
        } else {
            None
        }
    }

    pub fn get_project_summary(&self) -> String {
        if self.files.is_empty() {
            return "No files in context".to_string();
        }

        let mut summary_parts = vec![];
        summary_parts.push(format!("# {} Context Summary", self.cube_name));
        summary_parts.push(format!("## Files: {}", self.metadata.file_count));
        summary_parts.push(format!("## Total Size: {} bytes", self.metadata.total_size_bytes));
        summary_parts.push(format!("## Last Updated: {}", self.metadata.updated_at));
        summary_parts.push("
## File Summaries:
".to_string());

        for (file_path, file) in &self.files {
            summary_parts.push(format!("### {}
{}
", file_path, file.summary));
        }

        summary_parts.join("
")
    }

    pub fn scan_directory(&mut self, dir_path: &str) -> Result<usize> {
        let mut scanned_count = 0;
        self.scan_recursive(dir_path, &mut scanned_count)?;
        self.metadata.last_scan = chrono::Utc::now().to_rfc3339();
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(scanned_count)
    }

    fn scan_recursive(&mut self, dir_path: &str, scanned_count: &mut usize) -> Result<()> {
        let entries = fs::read_dir(dir_path)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                if !self.config.exclude_patterns.iter().any(|pattern| dir_name.contains(pattern)) {
                    self.scan_recursive(path.to_str().unwrap(), scanned_count)?;
                }
            } else if path.is_file() {
                let file_name = path.file_name().unwrap().to_string_lossy();
                let should_include = self.config.include_patterns.iter().any(|pattern| {
                    file_name.ends_with(pattern.trim_start_matches('*'))
                });
                let should_exclude = self.config.exclude_patterns.iter().any(|pattern| {
                    path.to_string_lossy().contains(pattern.trim_end_matches('/'))
                });

                if should_include && !should_exclude && *scanned_count < self.config.max_files {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let file_path = path.to_str().unwrap().to_string();
                        self.add_file(&file_path, &content);
                        *scanned_count += 1;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content).context("Failed to write context cube")
    }

    pub fn load(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content).context("Failed to parse context cube")
        } else {
            Ok(ContextCube::new("default", "Default Context"))
        }
    }
}

/// 文件上下文
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileContext {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub last_modified: String,
    pub summary: String,
}

fn generate_summary(content: &str, max_length: usize) -> String {
    if content.len() <= max_length {
        return content.to_string();
    }

    // 提取前几行和关键信息
    let lines: Vec<&str> = content.lines().take(5).collect();
    let first_part = lines.join("
");
    
    if first_part.len() <= max_length {
        return first_part;
    }

    // 如果还是太长，截断
    first_part[..max_length.min(first_part.len())].to_string()
}