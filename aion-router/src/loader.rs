use std::{
    fs,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, Result};

use aion_types::{
    capability_registry::CapabilityRegistry,
    types::{RouterPaths, SkillDefinition, SkillMetadata, SkillSource},
};

// ── Skills cache with TTL-based hot-reload ────────────────────────────────────

struct SkillsCache {
    skills: Vec<SkillDefinition>,
    loaded_at: SystemTime,
    workspace: std::path::PathBuf,
}

static SKILLS_CACHE: OnceLock<Mutex<Option<SkillsCache>>> = OnceLock::new();

fn skills_cache() -> &'static Mutex<Option<SkillsCache>> {
    SKILLS_CACHE.get_or_init(|| Mutex::new(None))
}

/// Cache TTL: re-scan skills/ directory every 30 seconds.
const CACHE_TTL: Duration = Duration::from_secs(30);

pub struct Loader;

impl Loader {
    pub fn load_local_skills(
        paths: &RouterPaths,
        capability_registry: &CapabilityRegistry,
    ) -> Result<Vec<SkillDefinition>> {
        // Check cache validity
        if let Ok(mut guard) = skills_cache().lock() {
            if let Some(ref cached) = *guard {
                let age = SystemTime::now().duration_since(cached.loaded_at).unwrap_or_default();
                if age < CACHE_TTL && cached.workspace == paths.workspace_root {
                    return Ok(cached.skills.clone());
                }
            }
            // Cache miss or expired — reload from disk
            let skills = Self::scan_skills_dir(paths, capability_registry)?;
            *guard = Some(SkillsCache {
                skills: skills.clone(),
                loaded_at: SystemTime::now(),
                workspace: paths.workspace_root.clone(),
            });
            return Ok(skills);
        }
        // Lock failed — fall through to direct scan
        Self::scan_skills_dir(paths, capability_registry)
    }

    /// Force invalidate the cache (e.g. after a skill is added/removed).
    pub fn invalidate_cache() {
        if let Ok(mut guard) = skills_cache().lock() {
            *guard = None;
        }
    }

    /// Load a single skill definition from its directory.
    ///
    /// Returns `Err` for any per-skill problem (unreadable/malformed
    /// `skill.json`, empty capabilities, invalid capability names) so the
    /// caller can skip just this skill instead of failing the whole scan.
    fn load_skill_dir(
        skill_dir: &std::path::Path,
        capability_registry: &CapabilityRegistry,
    ) -> Result<SkillDefinition> {
        let skill_json = skill_dir.join("skill.json");
        let metadata: SkillMetadata = serde_json::from_slice(&fs::read(&skill_json)?)?;
        if metadata.capabilities.is_empty() {
            return Err(anyhow!("skill {} declares no capabilities", metadata.name));
        }
        for capability in &metadata.capabilities {
            capability_registry.validate_name(capability)?;
        }
        Ok(SkillDefinition {
            metadata,
            root_dir: skill_dir.to_path_buf(),
            source: SkillSource::Local,
        })
    }

    fn scan_skills_dir(paths: &RouterPaths, capability_registry: &CapabilityRegistry) -> Result<Vec<SkillDefinition>> {
        if !paths.skills_dir.exists() {
            return Ok(Vec::new());
        }
        let mut skills = Vec::new();
        for entry in fs::read_dir(&paths.skills_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let skill_dir = entry.path();
            if !skill_dir.join("skill.json").exists() {
                continue;
            }
            // 单个技能损坏只跳过该技能并告警，不中断整个扫描：
            // 否则一个非法 skill.json 会让所有技能（含 MCP 工具）全部不可用。
            match Self::load_skill_dir(&skill_dir, capability_registry) {
                Ok(skill) => skills.push(skill),
                Err(err) => tracing::warn!(
                    skill_dir = %skill_dir.display(),
                    error = %err,
                    "skipping invalid skill definition"
                ),
            }
        }
        skills.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));
        Ok(skills)
    }
}
