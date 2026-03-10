use crate::models::{Registry, SkillMetadata};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct RegistryManager;

impl RegistryManager {
    pub fn load_registry<P: AsRef<Path>>(registry_path: P) -> Result<Registry> {
        if !registry_path.as_ref().exists() {
            let registry = Registry {
                skills: HashMap::new(),
            };
            return Ok(registry);
        }

        let content = fs::read_to_string(registry_path)?;
        if content.trim().is_empty() {
            let registry = Registry {
                skills: HashMap::new(),
            };
            return Ok(registry);
        }

        let registry: Registry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    pub fn save_registry<P: AsRef<Path>>(registry_path: P, registry: &Registry) -> Result<()> {
        let content = serde_json::to_string_pretty(registry)?;
        if let Some(parent) = registry_path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(registry_path, content)?;
        Ok(())
    }

    pub fn update_skill(registry: &mut Registry, skill_meta: SkillMetadata) {
        if let Some(existing) = registry.skills.get_mut(&skill_meta.name) {
            // Keep usage and lifecycle from registry
            let mut updated = skill_meta;
            updated.usage = existing.usage.clone();
            updated.lifecycle = existing.lifecycle.clone();
            *existing = updated;
        } else {
            registry.skills.insert(skill_meta.name.clone(), skill_meta);
        }
    }
}
