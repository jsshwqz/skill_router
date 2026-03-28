use std::{fs, path::PathBuf};

use anyhow::Result;
use serde::Deserialize;

use aion_types::types::{PermissionSet, RouterPaths, SkillDefinition, SkillMetadata, SkillSource};

#[derive(Debug, Deserialize)]
struct TrustedSourcesFile {
    #[serde(default)]
    sources: Vec<TrustedSource>,
}

#[derive(Debug, Deserialize)]
struct TrustedSource {
    name: String,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteCatalog {
    #[serde(default)]
    skills: Vec<RemoteSkillDefinition>,
}

#[derive(Debug, Deserialize)]
struct RemoteSkillDefinition {
    name: String,
    version: String,
    capabilities: Vec<String>,
    entrypoint: String,
    #[serde(default)]
    permissions: PermissionSet,
}

pub struct TrustedSourceSearch;

impl TrustedSourceSearch {
    /// Search trusted sources for skills matching the given capability.
    ///
    /// Reads local catalog files synchronously (fast) and fetches remote catalogs
    /// via async HTTP when URL-based sources are configured.
    pub async fn search_async(paths: &RouterPaths, capability: &str) -> Result<Vec<SkillDefinition>> {
        if !paths.trusted_sources_path.exists() {
            return Ok(Vec::new());
        }

        let config: TrustedSourcesFile =
            serde_json::from_slice(&fs::read(&paths.trusted_sources_path)?)?;
        let mut results = Vec::new();

        for source in config.sources {
            if let Some(path) = source.path {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    paths.workspace_root.join(path)
                };
                if !absolute.exists() {
                    continue;
                }
                let catalog: RemoteCatalog = serde_json::from_slice(&fs::read(&absolute)?)?;
                for skill in catalog
                    .skills
                    .into_iter()
                    .filter(|skill| skill.capabilities.iter().any(|item| item == capability))
                {
                    results.push(SkillDefinition {
                        metadata: SkillMetadata {
                            name: skill.name,
                            version: skill.version,
                            capabilities: skill.capabilities,
                            entrypoint: skill.entrypoint,
                            permissions: skill.permissions,
                            instruction: None,
                        },
                        root_dir: absolute
                            .parent()
                            .unwrap_or(&paths.workspace_root)
                            .to_path_buf(),
                        source: SkillSource::RemoteCandidate,
                    });
                }
            } else if let Some(url) = source.url {
                if !url.starts_with("https://") {
                    continue;
                }
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()?;
                let response = client.get(&url).send().await?;
                if !response.status().is_success() {
                    continue;
                }
                let catalog: RemoteCatalog = response.json().await?;
                for skill in catalog
                    .skills
                    .into_iter()
                    .filter(|skill| skill.capabilities.iter().any(|item| item == capability))
                {
                    results.push(SkillDefinition {
                        metadata: SkillMetadata {
                            name: format!("{}::{}", source.name, skill.name),
                            version: skill.version,
                            capabilities: skill.capabilities,
                            entrypoint: skill.entrypoint,
                            permissions: skill.permissions,
                            instruction: None,
                        },
                        root_dir: paths.workspace_root.clone(),
                        source: SkillSource::RemoteCandidate,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Synchronous search — only reads local file-based sources (no HTTP).
    /// Used by callers that cannot be async (e.g., Loader).
    pub fn search(paths: &RouterPaths, capability: &str) -> Result<Vec<SkillDefinition>> {
        if !paths.trusted_sources_path.exists() {
            return Ok(Vec::new());
        }

        let config: TrustedSourcesFile =
            serde_json::from_slice(&fs::read(&paths.trusted_sources_path)?)?;
        let mut results = Vec::new();

        for source in config.sources {
            if let Some(path) = source.path {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    paths.workspace_root.join(path)
                };
                if !absolute.exists() {
                    continue;
                }
                let catalog: RemoteCatalog = serde_json::from_slice(&fs::read(&absolute)?)?;
                for skill in catalog
                    .skills
                    .into_iter()
                    .filter(|skill| skill.capabilities.iter().any(|item| item == capability))
                {
                    results.push(SkillDefinition {
                        metadata: SkillMetadata {
                            name: skill.name,
                            version: skill.version,
                            capabilities: skill.capabilities,
                            entrypoint: skill.entrypoint,
                            permissions: skill.permissions,
                            instruction: None,
                        },
                        root_dir: absolute
                            .parent()
                            .unwrap_or(&paths.workspace_root)
                            .to_path_buf(),
                        source: SkillSource::RemoteCandidate,
                    });
                }
            }
            // URL-based sources skipped in sync path
        }

        Ok(results)
    }
}
