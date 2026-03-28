//! 技能注册中心
//!
//! 搜索、安装和发布社区技能包。

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 远程注册表源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySource {
    /// 源名称
    pub name: String,
    /// 注册表 URL（JSON API）
    pub url: String,
    /// 是否受信任
    #[serde(default)]
    pub trusted: bool,
}

/// 远程技能条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSkillEntry {
    /// 技能名称
    pub name: String,
    /// 版本
    pub version: String,
    /// 描述
    pub description: String,
    /// 下载 URL
    pub download_url: String,
    /// 提供的能力
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// 来源注册表
    #[serde(default)]
    pub source: String,
    /// SHA256 校验和
    #[serde(default)]
    pub checksum: Option<String>,
}

/// 技能注册中心
pub struct RegistryHub {
    /// 已配置的注册表源
    sources: Vec<RegistrySource>,
    /// 本地技能目录
    skills_dir: PathBuf,
}

impl RegistryHub {
    /// 创建注册中心
    pub fn new(skills_dir: &Path) -> Self {
        Self {
            sources: vec![
                RegistrySource {
                    name: "aion-forge-hub".to_string(),
                    url: "https://hub.aion-forge.dev/api/v1/skills".to_string(),
                    trusted: true,
                },
            ],
            skills_dir: skills_dir.to_path_buf(),
        }
    }

    /// 从配置文件加载自定义源
    pub fn load_sources(config_path: &Path) -> Result<Vec<RegistrySource>> {
        if !config_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(config_path)?;
        let sources: Vec<RegistrySource> = serde_json::from_str(&content)?;
        Ok(sources)
    }

    /// 添加注册表源
    pub fn add_source(&mut self, source: RegistrySource) {
        self.sources.push(source);
    }

    /// 搜索远程注册表
    pub async fn search(&self, query: &str) -> Result<Vec<RemoteSkillEntry>> {
        let mut results = Vec::new();

        for source in &self.sources {
            match Self::search_source(source, query).await {
                Ok(mut entries) => {
                    for entry in &mut entries {
                        entry.source = source.name.clone();
                    }
                    results.extend(entries);
                }
                Err(e) => {
                    tracing::warn!(
                        source = %source.name,
                        error = %e,
                        "registry hub: failed to search source"
                    );
                }
            }
        }

        Ok(results)
    }

    /// 搜索单个源
    async fn search_source(source: &RegistrySource, query: &str) -> Result<Vec<RemoteSkillEntry>> {
        let url = format!("{}?q={}", source.url, urlencoding::encode(query));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let resp = client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "registry returned status {}",
                resp.status()
            ));
        }

        let entries: Vec<RemoteSkillEntry> = resp.json().await?;
        Ok(entries)
    }

    /// 安装远程技能到本地
    pub async fn install(&self, entry: &RemoteSkillEntry) -> Result<PathBuf> {
        let skill_dir = self.skills_dir.join(&entry.name);
        std::fs::create_dir_all(&skill_dir)?;

        tracing::info!(
            skill = %entry.name,
            version = %entry.version,
            source = %entry.source,
            "registry hub: installing skill"
        );

        // 下载技能包
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        let resp = client.get(&entry.download_url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "download failed: status {}",
                resp.status()
            ));
        }

        let bytes = resp.bytes().await?;

        // 如果有校验和，验证
        if let Some(ref expected_hash) = entry.checksum {
            let actual_hash = simple_hash(&bytes);
            if actual_hash != *expected_hash {
                std::fs::remove_dir_all(&skill_dir)?;
                return Err(anyhow::anyhow!(
                    "checksum mismatch: expected {}, got {}",
                    expected_hash,
                    actual_hash
                ));
            }
        }

        // 保存到本地（假设是 JSON 技能包）
        std::fs::write(skill_dir.join("skill-package.json"), &bytes)?;

        tracing::info!(
            skill = %entry.name,
            path = %skill_dir.display(),
            "registry hub: skill installed"
        );

        Ok(skill_dir)
    }

    /// 列出已安装的技能
    pub fn list_installed(&self) -> Result<Vec<String>> {
        let mut installed = Vec::new();
        if self.skills_dir.exists() {
            for entry in std::fs::read_dir(&self.skills_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        installed.push(name.to_string());
                    }
                }
            }
        }
        Ok(installed)
    }
}

/// 简易哈希（与 aion-sandbox policy.rs 中一致）
fn simple_hash(data: &[u8]) -> String {
    let mut hash = 0u64;
    for (i, byte) in data.iter().enumerate() {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as u64).wrapping_add(i as u64);
    }
    format!("{:016x}", hash)
}

/// URL 编码辅助模块
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut encoded = String::new();
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char);
                }
                _ => {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_hub_new() {
        let tmp = std::env::temp_dir().join("aion-hub-test");
        let hub = RegistryHub::new(&tmp);
        assert_eq!(hub.sources.len(), 1);
        assert_eq!(hub.sources[0].name, "aion-forge-hub");
    }

    #[test]
    fn test_remote_skill_entry_serde() {
        let entry = RemoteSkillEntry {
            name: "curl-wrapper".to_string(),
            version: "1.0.0".to_string(),
            description: "HTTP tool".to_string(),
            download_url: "https://hub.aion-forge.dev/skills/curl-wrapper.zip".to_string(),
            capabilities: vec!["http_fetch".to_string()],
            source: "official".to_string(),
            checksum: Some("abc123".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: RemoteSkillEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "curl-wrapper");
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding::encode("hello world"), "hello%20world");
        assert_eq!(urlencoding::encode("a+b=c"), "a%2Bb%3Dc");
    }

    #[test]
    fn test_list_installed_empty() {
        let tmp = std::env::temp_dir().join("aion-hub-list-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let hub = RegistryHub::new(&tmp);
        let installed = hub.list_installed().unwrap();
        assert!(installed.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
