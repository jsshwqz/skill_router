use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::context_cube::{ContextCube, ContextCubeConfig, ContextCubeMetadata};

/// 多上下文立方体管理器
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiContextManager {
    pub contexts: HashMap<String, ContextCube>,
    pub shared_contexts: Vec<String>,
    pub global_search_enabled: bool,
}

impl MultiContextManager {
    pub fn new() -> Self {
        MultiContextManager {
            contexts: HashMap::new(),
            shared_contexts: Vec::new(),
            global_search_enabled: true,
        }
    }

    pub fn create_context(&mut self, context_id: &str, context_name: &str) -> Result<()> {
        let context = ContextCube::new(context_id, context_name);
        self.contexts.insert(context_id.to_string(), context);
        Ok(())
    }

    pub fn delete_context(&mut self, context_id: &str) -> bool {
        self.contexts.remove(context_id).is_some()
    }

    pub fn get_context(&self, context_id: &str) -> Option<&ContextCube> {
        self.contexts.get(context_id)
    }

    pub fn get_context_mut(&mut self, context_id: &str) -> Option<&mut ContextCube> {
        self.contexts.get_mut(context_id)
    }

    pub fn list_contexts(&self) -> Vec<(&str, &ContextCube)> {
        self.contexts.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    pub fn set_shared(&mut self, context_id: &str, shared: bool) {
        if let Some(context) = self.contexts.get_mut(context_id) {
            context.metadata.is_shared = shared;
            if shared && !self.shared_contexts.contains(&context_id.to_string()) {
                self.shared_contexts.push(context_id.to_string());
            } else if !shared {
                self.shared_contexts.retain(|s| s != context_id);
            }
        }
    }

    pub fn add_file_to_context(&mut self, context_id: &str, file_path: &str, content: &str) -> Result<()> {
        if let Some(context) = self.contexts.get_mut(context_id) {
            context.add_file(file_path, content);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Context not found: {}", context_id))
        }
    }

    pub fn scan_context(&mut self, context_id: &str, dir_path: &str) -> Result<usize> {
        if let Some(context) = self.contexts.get_mut(context_id) {
            context.scan_directory(dir_path)
        } else {
            Err(anyhow::anyhow!("Context not found: {}", context_id))
        }
    }

    pub fn get_context_summary(&self, context_id: &str) -> Result<String> {
        if let Some(context) = self.contexts.get(context_id) {
            Ok(context.get_project_summary())
        } else {
            Err(anyhow::anyhow!("Context not found: {}", context_id))
        }
    }

    pub fn search_context(&self, context_id: &str, keyword: &str) -> Result<Vec<(String, String)>> {
        if let Some(context) = self.contexts.get(context_id) {
            let mut results = Vec::new();
            for (file_path, file) in &context.files {
                if file.content.contains(keyword) || file.summary.contains(keyword) {
                    results.push((file_path.clone(), file.summary.clone()));
                }
            }
            Ok(results)
        } else {
            Err(anyhow::anyhow!("Context not found: {}", context_id))
        }
    }

    pub fn global_search(&self, keyword: &str) -> Vec<(&str, Vec<(String, String)>)> {
        self.contexts
            .iter()
            .map(|(context_id, context)| {
                let results: Vec<(String, String)> = context
                    .files
                    .iter()
                    .filter(|(_, file)| file.content.contains(keyword) || file.summary.contains(keyword))
                    .map(|(path, file)| (path.clone(), file.summary.clone()))
                    .collect();
                (context_id.as_str(), results)
            })
            .collect()
    }

    pub fn save_all(&self) -> Result<()> {
        // 创建 contexts 目录
        fs::create_dir_all("CONTEXT/cubes")?;
        
        for (context_id, context) in &self.contexts {
            context.save(&format!("CONTEXT/cubes/{}.json", context_id))?;
        }
        
        // 保存管理器元数据
        let meta = serde_json::to_string_pretty(self)?;
        fs::write("CONTEXT/multicontext_manager.json", meta)?;
        Ok(())
    }

    pub fn load_all() -> Result<Self> {
        let manager_path = "CONTEXT/multicontext_manager.json";
        if Path::new(manager_path).exists() {
            let content = fs::read_to_string(manager_path)?;
            serde_json::from_str(&content).context("Failed to parse multi-context manager")
        } else {
            Ok(MultiContextManager::new())
        }
    }
}