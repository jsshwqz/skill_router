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

use anyhow::Result;
use models::Config;
use std::path::Path;
use std::fs;

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_config<P: AsRef<Path>>(path: P, config: &Config) -> Result<()> {
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}
