use std::path::PathBuf;
use anyhow::{Result, anyhow};
use aion_types::types::{SkillDefinition, SkillMetadata, SkillSource, RouterPaths};
use aion_types::ai_native::AiNativePayload;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryLayer {
    Local,
    Project,
    Central,
}

#[derive(Debug, Clone)]
pub struct DiscoveryMatch {
    pub skill: SkillDefinition,
    pub layer: DiscoveryLayer,
    pub confidence: f64,
}

pub struct DiscoveryRadar {
    pub paths: RouterPaths,
    pub project_paths: Vec<PathBuf>,
}

impl DiscoveryRadar {
    pub fn new(paths: RouterPaths) -> Self {
        Self { 
            paths,
            project_paths: Vec::new(),
        }
    }

    pub fn with_project_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.project_paths = paths;
        self
    }

    /// performs a cascading search for a capability across multiple layers.
    pub fn cascade_search(&self, capability: &str) -> Result<Option<DiscoveryMatch>> {
        // ... (existing cascade_search implementation) ...
        // I will keep the original implementation but use a helper to avoid duplication
        self.perform_cascade(capability)
    }

    /// performs a cascading search based on an AI-Native Payload's intent or capability.
    pub fn search_by_payload(&self, payload: &AiNativePayload) -> Result<Option<DiscoveryMatch>> {
        let target = payload.capability.as_deref().unwrap_or(&payload.intent);
        self.perform_cascade(target)
    }

    fn perform_cascade(&self, target: &str) -> Result<Option<DiscoveryMatch>> {
        // 1. Local Layer: Check the current workspace's skills directory
        if let Some(skill) = self.search_local(target)? {
            return Ok(Some(DiscoveryMatch {
                skill,
                layer: DiscoveryLayer::Local,
                confidence: 1.0,
            }));
        }

        // 2. Project Layer: Check linked project paths (Phase 2.2)
        if let Some(skill) = self.search_project(target)? {
            return Ok(Some(DiscoveryMatch {
                skill,
                layer: DiscoveryLayer::Project,
                confidence: 0.9,
            }));
        }

        // 3. Central Layer: Check the registry/central store (Phase 2.3)
        if let Some(skill) = self.search_central(target)? {
            return Ok(Some(DiscoveryMatch {
                skill,
                layer: DiscoveryLayer::Central,
                confidence: 0.8,
            }));
        }

        Ok(None)
    }

    fn search_local(&self, capability: &str) -> Result<Option<SkillDefinition>> {
        self.search_in_dir(&self.paths.skills_dir, capability, SkillSource::Local)
    }

    fn search_project(&self, capability: &str) -> Result<Option<SkillDefinition>> {
        for path in &self.project_paths {
            if let Some(skill) = self.search_in_dir(path, capability, SkillSource::Local)? {
                return Ok(Some(skill));
            }
        }
        Ok(None)
    }

    fn search_in_dir(&self, dir: &std::path::Path, capability: &str, source: SkillSource) -> Result<Option<SkillDefinition>> {
        if !dir.exists() {
            return Ok(None);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Ok(mut skill) = self.load_skill_at(&path) {
                    skill.source = source; // Override source based on layer
                    if skill.supports_capability(capability) {
                        return Ok(Some(skill));
                    }
                }
            }
        }
        Ok(None)
    }

    fn search_central(&self, capability: &str) -> Result<Option<SkillDefinition>> {
        // [Phase 2.3] Mocking a central fetching behavior
        // In reality, this would be an HTTP call to a central registry.
        // For now, we simulate finding specific "pro" capabilities.
        if capability == "advanced_reasoning" || capability == "multi_modal_analysis" {
            return Ok(Some(SkillDefinition {
                metadata: SkillMetadata {
                    name: format!("remote-{}", capability),
                    version: "0.1.0".to_string(),
                    capabilities: vec![capability.to_string()],
                    entrypoint: "entry.rs".to_string(),
                    permissions: aion_types::types::PermissionSet::default(),
                    instruction: None,
                },
                root_dir: PathBuf::from(".skill-router/remote-cache"), // Placeholder path
                source: SkillSource::RemoteCandidate,
            }));
        }
        Ok(None)
    }

    fn load_skill_at(&self, path: &std::path::Path) -> Result<SkillDefinition> {
        let metadata_path = path.join("skill.json");
        if !metadata_path.exists() {
            return Err(anyhow!("No skill.json found at {:?}", path));
        }

        let content = std::fs::read_to_string(&metadata_path)?;
        let metadata: SkillMetadata = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse skill.json at {:?}: {}", path, e))?;

        Ok(SkillDefinition {
            metadata,
            root_dir: path.to_path_buf(),
            source: SkillSource::Local,
        })
    }
}
