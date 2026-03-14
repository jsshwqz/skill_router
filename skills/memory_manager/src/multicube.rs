use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::memcube::{CubeConfig, CubeMetadata, MemoryCube, MemoryEntry};

/// 多记忆立方体管理器（支持立方体间隔离、共享、组合）
#[derive(Debug, Serialize, Deserialize)]
pub struct MultiCubeManager {
    pub cubes: HashMap<String, MemoryCube>,
    pub shared_cubes: Vec<String>, // 共享立方体 ID 列表
    pub global_search: bool,       // 是否全局搜索
}

impl MultiCubeManager {
    /// 创建新的多立方体管理器
    pub fn new() -> Self {
        MultiCubeManager {
            cubes: HashMap::new(),
            shared_cubes: Vec::new(),
            global_search: true,
        }
    }

    /// 创建新立方体
    pub fn create_cube(
        &mut self,
        cube_id: &str,
        cube_name: &str,
        config: CubeConfig,
        metadata: CubeMetadata,
    ) {
        let mut cube = MemoryCube::new(cube_id, cube_name);
        cube.config = config;
        cube.metadata = metadata;
        self.cubes.insert(cube_id.to_string(), cube);
    }

    /// 删除立方体
    pub fn delete_cube(&mut self, cube_id: &str) -> bool {
        self.cubes.remove(cube_id).is_some()
    }

    /// 列出所有立方体
    pub fn list_cubes(&self) -> Vec<(&str, &MemoryCube)> {
        self.cubes.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    /// 获取立方体
    pub fn get_cube(&self, cube_id: &str) -> Option<&MemoryCube> {
        self.cubes.get(cube_id)
    }

    /// 设置立方体共享
    pub fn set_shared(&mut self, cube_id: &str, shared: bool) {
        if let Some(cube) = self.cubes.get_mut(cube_id) {
            cube.metadata.is_shared = shared;
            if shared && !self.shared_cubes.contains(&cube_id.to_string()) {
                self.shared_cubes.push(cube_id.to_string());
            } else if !shared {
                self.shared_cubes.retain(|s| s != cube_id);
            }
        }
    }

    /// 在立方体中保存记忆
    pub fn save_memory(&mut self, cube_id: &str, entry: MemoryEntry) -> Result<()> {
        if let Some(cube) = self.cubes.get_mut(cube_id) {
            cube.add_memory(entry);
            cube.save(&format!("MEMORY/cubes/{}.json", cube_id))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Cube not found: {}", cube_id))
        }
    }

    /// 从立方体检索记忆
    pub fn retrieve(
        &self,
        cube_id: &str,
        keyword: &str,
        top_k: usize,
    ) -> Result<Vec<&MemoryEntry>> {
        if let Some(cube) = self.cubes.get(cube_id) {
            Ok(cube.retrieve(keyword, top_k))
        } else {
            Err(anyhow::anyhow!("Cube not found: {}", cube_id))
        }
    }

    /// 全局检索（跨所有立方体）
    pub fn global_retrieve(&self, keyword: &str, top_k: usize) -> Vec<(&str, Vec<&MemoryEntry>)> {
        self.cubes
            .iter()
            .map(|(cube_id, cube)| {
                let results = cube.retrieve(keyword, top_k);
                (cube_id.as_str(), results)
            })
            .collect()
    }

    /// 保存所有立方体
    pub fn save_all(&self) -> Result<()> {
        for (cube_id, cube) in &self.cubes {
            cube.save(&format!("MEMORY/cubes/{}.json", cube_id))?;
        }
        // 保存管理器元数据
        let meta = serde_json::to_string_pretty(self)?;
        fs::write("MEMORY/multicube_manager.json", meta)?;
        Ok(())
    }

    /// 加载所有立方体
    pub fn load_all() -> Result<Self> {
        let manager_path = "MEMORY/multicube_manager.json";
        if Path::new(manager_path).exists() {
            let content = fs::read_to_string(manager_path)?;
            serde_json::from_str(&content).context("Failed to parse multi-cube manager")
        } else {
            Ok(MultiCubeManager::new())
        }
    }
}
