pub mod models;
pub mod planner;
pub mod loader;
pub mod registry;
pub mod matcher;
pub mod executor;
pub mod security;
pub mod lifecycle;
pub mod online_search;
pub mod synth;
pub mod security_analyzer;
pub mod skills_finder;

use anyhow::Result;
use models::Config;
use std::path::Path;
use std::fs;

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        // Return a default config if file exists but is empty
        let config = Config {
            enable_auto_install: false,
            skills_dir: "skills".to_string(),
            registry_file: "registry.json".to_string(),
            logs_dir: "logs".to_string(),
            trusted_sources: vec!["https://github.com/trusted-source".to_string()],
            llm_enabled: Some(false),
            llm_command: None,
        };
        return Ok(config);
    }
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_config<P: AsRef<Path>>(path: P, config: &Config) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}
