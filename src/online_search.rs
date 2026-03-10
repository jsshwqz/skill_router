use crate::models::{SkillMetadata, Config};
use serde_json::Value;
use anyhow::{Result, Context};
use std::path::Path;
use std::fs;
use std::process::Command;

/// OnlineSearch - 纯 Rust 实现的在线技能搜索模块
///
/// v0.0.1 特性：
/// - 使用 reqwest 进行 GitHub API 搜索
/// - 集成 Rust 安全审计
/// - 自动技能元数据验证
pub struct OnlineSearch;

impl OnlineSearch {
    /// 在线搜索技能
    /// 
    /// 搜索策略：
    /// 1. 尝试通过 GitHub API 搜索相关仓库
    /// 2. 验证技能元数据
    /// 3. 执行安全审计
    pub async fn search(
        config: &Config,
        capability: &str,
        _task: &str
    ) -> Result<Option<SkillMetadata>> {
        println!("[ONLINE] 🔍 Searching for capability '{}'...", capability);

        // 尝试 GitHub 搜索
        if let Some(skill) = Self::search_github(config, capability, "").await? {
            return Ok(Some(skill));
        }

        println!("[ONLINE] No suitable skill found on GitHub");
        Ok(None)
    }

    /// 通过 GitHub API 搜索技能仓库
    async fn search_github(
        config: &Config,
        capability: &str,
        _task: &str
    ) -> Result<Option<SkillMetadata>> {
        // 构造 GitHub API 查询
        let query = format!(
            "{} topic:skill-router language:rust OR language:python",
            capability.replace("_", "-")
        );

        println!("[ONLINE] GitHub query: {}", query);

        // 使用 reqwest 调用 GitHub API
        let client = reqwest::Client::builder()
            .user_agent("Skill-Router/0.0.1")
            .build()
            .context("Failed to create HTTP client")?;

        let url = format!(
            "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page=5",
            urlencoding::encode(&query)
        );

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to GitHub API")?;

        if !response.status().is_success() {
            println!("[ONLINE] GitHub API returned status: {}", response.status());
            return Ok(None);
        }

        let json: Value = response
            .json()
            .await
            .context("Failed to parse GitHub API response")?;

        let empty_vec: Vec<Value> = vec![];
        let items = json["items"].as_array().unwrap_or(&empty_vec);

        if items.is_empty() {
            println!("[ONLINE] No repositories found on GitHub");
            return Ok(None);
        }

        // 遍历搜索结果，尝试安装和验证
        for repo in items.iter().take(3) {
            if let Some(repo_name) = repo["name"].as_str() {
                if let Some(clone_url) = repo["clone_url"].as_str() {
                    println!("[ONLINE] Found candidate: {} from {}", repo_name, clone_url);

                    if let Some(skill) = Self::try_install_and_validate(
                        config,
                        repo_name,
                        clone_url,
                        capability,
                        repo["description"].as_str().unwrap_or("")
                    ).await? {
                        return Ok(Some(skill));
                    }
                }
            }
        }

        Ok(None)
    }

    /// 尝试安装和验证技能
    async fn try_install_and_validate(
        config: &Config,
        repo_name: &str,
        clone_url: &str,
        capability: &str,
        _description: &str
    ) -> Result<Option<SkillMetadata>> {
        let skill_dir = Path::new(&config.skills_dir).join(repo_name);

        println!("[ONLINE] Installing {} to {:?}", repo_name, skill_dir);

        // 如果技能目录已存在，跳过克隆
        if !skill_dir.exists() {
            // 使用 git clone 下载仓库
            let output = Command::new("git")
                .arg("clone")
                .arg(clone_url)
                .arg(&skill_dir)
                .output()
                .context("Failed to execute git clone")?;

            if !output.status.success() {
                println!("[ONLINE] Git clone failed: {}", String::from_utf8_lossy(&output.stderr));
                return Ok(None);
            }
        }

        // 读取 skill.json
        let skill_json_path = skill_dir.join("skill.json");
        if !skill_json_path.exists() {
            println!("[ONLINE] skill.json not found in {:?}", skill_dir);
            let _ = fs::remove_dir_all(&skill_dir);
            return Ok(None);
        }

        let skill_content = fs::read_to_string(&skill_json_path)
            .context("Failed to read skill.json")?;

        let skill_meta: SkillMetadata = serde_json::from_str(&skill_content)
            .context("Failed to parse skill.json")?;

        // 验证技能是否提供所需能力
        if !skill_meta.capabilities.contains(&capability.to_string()) {
            println!("[ONLINE] Skill '{}' does not provide capability '{}'", skill_meta.name, capability);
            let _ = fs::remove_dir_all(&skill_dir);
            return Ok(None);
        }

        // 执行安全审计
        println!("[ONLINE] Running security audit on {:?}...", skill_dir);
        match crate::security_analyzer::SecurityAnalyzer::audit_skill_dir(&skill_dir) {
            Ok(_) => {
                println!("[ONLINE] ✅ Security audit passed for '{}'", skill_meta.name);
                Ok(Some(skill_meta))
            }
            Err(e) => {
                eprintln!("[ONLINE] ❌ Security audit failed for '{}': {}", skill_meta.name, e);
                let _ = fs::remove_dir_all(&skill_dir);
                Ok(None)
            }
        }
    }

    /// 同步版本（用于向后兼容）
    /// 注意：这个方法内部会创建异步运行时，在实际使用中建议直接调用异步版本
    pub fn search_sync(config: &Config, capability: &str, task: &str) -> Option<SkillMetadata> {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(Self::search(config, capability, task)).ok().flatten()
    }}