use crate::models::{Config, SkillMetadata};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

/// 搜索回退响应结构
#[derive(Debug, Serialize, Deserialize)]
pub struct FallbackResponse {
    pub status: String,
    pub mode: String,
    pub reason: String,
    pub target_urls: Vec<String>,
    pub instruction: String,
}

/// 关键词裂变工具
pub struct QueryFission;

impl QueryFission {
    pub fn expand(query: &str) -> Vec<String> {
        let mut expanded = HashSet::new();
        expanded.insert(query.to_string());

        if query.contains("专利") || query.contains("发明") || query.contains("patent") {
            let base = query
                .replace("专利", "")
                .replace("发明", "")
                .replace("patent", "")
                .trim()
                .to_string();
            if !base.is_empty() {
                expanded.insert(format!("{} 发明专利", base));
                expanded.insert(format!("{} 知识产权", base));
                expanded.insert(format!("{} 企查查 专利", base));
            }
        } else if query.chars().count() <= 4 {
            expanded.insert(format!("{} 简介", query));
            expanded.insert(format!("{} 简历", query));
        }

        expanded.into_iter().collect()
    }
}

/// 智能搜索调度器 - 负责执行混合搜索并处理降级逻辑
pub struct SmartSearch;

impl SmartSearch {
    pub fn execute(query: &str) -> Result<Value> {
        let hybrid_exe = Path::new("skills/hybrid_search/target/release/hybrid_search.exe");
        
        // 如果二进制文件不存在，直接触发回退
        if !hybrid_exe.exists() {
            return Ok(serde_json::to_value(Self::trigger_browser_mode(query, "Hybrid search binary not found"))?);
        }

        // 尝试执行混合搜索
        let output = Command::new(hybrid_exe)
            .arg(query)
            .output();

        match output {
            Ok(res) => {
                let stdout = String::from_utf8_lossy(&res.stdout);
                if let Ok(data) = serde_json::from_str::<Value>(&stdout) {
                    // 检查是否成功且有结果
                    let is_success = data["status"].as_str() == Some("success");
                    let has_results = data["results"].as_array().map_or(false, |a| !a.is_empty());

                    if is_success && has_results {
                        return Ok(data);
                    }

                    // 检查错误信息
                    let error_msg = data["error"].as_str().unwrap_or("");
                    if error_msg.contains("handshake") || error_msg.contains("timeout") || error_msg.contains("Network error") {
                        return Ok(serde_json::to_value(Self::trigger_browser_mode(query, &format!("API受阻: {}", error_msg)))?);
                    }
                }
                Ok(serde_json::to_value(Self::trigger_browser_mode(query, "无有效结果"))?)
            }
            Err(e) => {
                Ok(serde_json::to_value(Self::trigger_browser_mode(query, &format!("执行异常: {}", e)))?)
            }
        }
    }

    fn trigger_browser_mode(query: &str, reason: &str) -> FallbackResponse {
        let encoded_q = urlencoding::encode(query);
        FallbackResponse {
            status: "fallback".to_string(),
            mode: "BROWSER_DIRECT_CAPTURE".to_string(),
            reason: reason.to_string(),
            target_urls: vec![
                format!("https://www.baidu.com/s?wd={}", encoded_q),
                format!("https://www.bing.com/search?q={}", encoded_q),
            ],
            instruction: "常规API已失效。请使用 browser_automation 模块直接访问上述 URL，翻页至第3页，提取所有相关链接。".to_string(),
        }
    }
}

/// OnlineSearch - 纯 Rust 实现的在线技能搜索模块
pub struct OnlineSearch;

impl OnlineSearch {
    /// 在线搜索技能
    pub async fn search(
        config: &Config,
        capability: &str,
        _task: &str,
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
        _task: &str,
    ) -> Result<Option<SkillMetadata>> {
        // 构造 GitHub API 查询
        let query = format!(
            "{} topic:skill-router language:rust OR language:python",
            capability.replace("_", "-")
        );

        println!("[ONLINE] GitHub query: {}", query);

        // 使用 reqwest 调用 GitHub API
        let client = reqwest::Client::builder()
            .user_agent("Skill-Router/0.2.1")
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
                        repo["description"].as_str().unwrap_or(""),
                    )
                    .await?
                    {
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
        _description: &str,
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
                println!(
                    "[ONLINE] Git clone failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
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

        let skill_content =
            fs::read_to_string(&skill_json_path).context("Failed to read skill.json")?;

        let skill_meta: SkillMetadata =
            serde_json::from_str(&skill_content).context("Failed to parse skill.json")?;

        // 验证技能是否提供所需能力
        if !skill_meta.capabilities.contains(&capability.to_string()) {
            println!(
                "[ONLINE] Skill '{}' does not provide capability '{}'",
                skill_meta.name, capability
            );
            let _ = fs::remove_dir_all(&skill_dir);
            return Ok(None);
        }

        // 执行安全审计
        println!("[ONLINE] Running security audit on {:?}...", skill_dir);
        match crate::security_analyzer::SecurityAnalyzer::audit_skill_dir(&skill_dir) {
            Ok(_) => {
                println!(
                    "[ONLINE] ✅ Security audit passed for '{}'",
                    skill_meta.name
                );
                Ok(Some(skill_meta))
            }
            Err(e) => {
                eprintln!(
                    "[ONLINE] ❌ Security audit failed for '{}': {}",
                    skill_meta.name, e
                );
                let _ = fs::remove_dir_all(&skill_dir);
                Ok(None)
            }
        }
    }

    /// 同步版本（用于向后兼容）
    pub fn search_sync(config: &Config, capability: &str, task: &str) -> Option<SkillMetadata> {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(Self::search(config, capability, task))
            .ok()
            .flatten()
    }
}

