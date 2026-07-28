//! 市场技能检索 builtin：market_search
//!
//! 三级降级策略：
//!   1. `npx skills find` — 最快，但不一定可用（依赖 Node.js 生态）
//!   2. `discovery_search` 级联搜索 — Google → HTTP → 本地可信源
//!   3. GitHub 技能相关结果过滤
//!
//! 全部失败则返回空结果，由调用方决定是否走 Synthesizer 新建。

use anyhow::Result;
use regex::Regex;
use serde_json::{json, Value};
use std::time::Duration;

use aion_intel::discovery_radar::{DiscoveryRadar, SearchHit};
use aion_types::types::{ExecutionContext, RouterPaths, SkillDefinition};

use super::BuiltinSkill;

/// 常见技能仓库关键词，用于从搜索结果中识别技能
const SKILL_INDICATORS: &[&str] = &[
    "claude skill",
    "claude-plugin",
    "skill for claude",
    "claude-code",
    "aion skill",
    "skill.json",
    "SKILL.md",
    "agent skill",
];

pub struct MarketSearch;

#[async_trait::async_trait]
impl BuiltinSkill for MarketSearch {
    fn name(&self) -> &'static str {
        "market_search"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let task = context.context["task"]
            .as_str()
            .or_else(|| context.context["text"].as_str())
            .or_else(|| context.context["query"].as_str())
            .unwrap_or(&context.task)
            .to_string();

        let mut candidates: Vec<Value> = Vec::new();
        let mut sources_tried: Vec<String> = Vec::new();
        let mut sources_succeeded: Vec<String> = Vec::new();

        // ── Level 1: npx skills find ──
        sources_tried.push("npx_skills".into());
        match try_npx_skills(&task).await {
            Ok(Some(hits)) if !hits.is_empty() => {
                sources_succeeded.push("npx_skills".into());
                candidates.extend(hits);
            }
            _ => {} // 静默降级
        }

        // ── Level 2: discovery_search 级联 ──
        sources_tried.push("discovery_search".into());
        let workspace = std::env::current_dir().unwrap_or_default();
        let paths = RouterPaths::for_workspace(&workspace);
        if let Ok(result) = DiscoveryRadar::cascade_search(&task, &paths).await {
            if !result.hits.is_empty() {
                sources_succeeded.push("discovery_search".into());
                for hit in &result.hits {
                    if let Some(candidate) = hit_to_candidate(hit) {
                        candidates.push(candidate);
                    }
                }
            }
        }

        // ── Level 3: 如果结果太少，用更广的关键词再搜一次 ──
        if candidates.len() < 3 {
            let broad_query = format!("claude skill for {} github", task);
            sources_tried.push("discovery_search_broad".into());
            if let Ok(result) = DiscoveryRadar::cascade_search(&broad_query, &paths).await {
                if !result.hits.is_empty() {
                    sources_succeeded.push("discovery_search_broad".into());
                    for hit in &result.hits {
                        let url_exists = candidates
                            .iter()
                            .any(|candidate| candidate["url"].as_str() == Some(&hit.url));
                        if !url_exists {
                            if let Some(candidate) = hit_to_candidate(hit) {
                                candidates.push(candidate);
                            }
                        }
                    }
                }
            }
        }

        Ok(json!({
            "task": task,
            "sources_tried": sources_tried,
            "sources_succeeded": sources_succeeded,
            "total_candidates": candidates.len(),
            "candidates": candidates,
            "summary": if candidates.is_empty() {
                "未在市场上找到匹配的技能。建议使用 Synthesizer 新建占位技能。".to_string()
            } else {
                format!("找到 {} 个候选技能。可通过 GitHub 仓库地址或 skill.json 接入。", candidates.len())
            }
        }))
    }
}

/// 尝试 `npx skills find <query>`，10 秒超时。
/// 内部清空代理环境变量，避免本地代理阻断 npm 请求。
async fn try_npx_skills(query: &str) -> Result<Option<Vec<Value>>> {
    let Ok(child) = tokio::process::Command::new("npx")
        .args(["skills", "find", query])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        // 清空可能导致 npm 挂起的代理变量
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .spawn()
    else {
        return Ok(None); // npx not found
    };

    let output = match tokio::time::timeout(Duration::from_secs(10), child.wait_with_output()).await {
        Ok(Ok(out)) if out.status.success() => out,
        _ => return Ok(None), // timeout or failure
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 剥离 ANSI 颜色码，得到纯文本行
    let ansi_re = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    let mut results = Vec::new();

    // npx skills find 输出格式：
    //   owner/repo@skill-name  N installs
    //   └ https://skills.sh/owner/repo/skill-name
    for line in stdout.lines() {
        let clean = ansi_re.replace_all(line, "");
        let line = clean.trim();
        // 匹配 owner/repo@skill 模式（不含空格、不含特殊字符开头）
        if let Some(rest) = line.strip_suffix(" installs") {
            if let Some(at_idx) = rest.rfind('@') {
                let repo_part = &rest[..at_idx];
                let skill_name = &rest[at_idx + 1..];
                let name = skill_name.trim();
                if !name.is_empty() {
                    results.push(json!({
                        "name": name,
                        "description": format!("{}/{}", repo_part.trim(), name),
                        "source": "npx_skills",
                        "url": format!("https://skills.sh/{}/{}", repo_part.trim(), name),
                        "confidence": 0.8,
                    }));
                }
            }
        }
    }

    Ok(Some(results))
}

/// 判断搜索结果是否像技能，如果是则返回格式化的候选记录。
fn hit_to_candidate(hit: &SearchHit) -> Option<Value> {
    let url_lower = hit.url.to_lowercase();
    let title_lower = hit.title.to_lowercase();

    // 判断是否为技能相关
    let is_skill = SKILL_INDICATORS
        .iter()
        .any(|&kw| title_lower.contains(kw) || url_lower.contains(kw));

    // GitHub 上的技能仓库也收录（即使标题不包含关键词）
    let is_github = url_lower.contains("github.com");

    if !is_skill && !is_github {
        return None;
    }

    // 计算可信度
    let mut confidence: f64 = 0.5;
    if is_skill {
        confidence += 0.2;
    }
    if url_lower.contains("github.com") {
        confidence += 0.1;
    }
    if !hit.snippet.is_empty() && hit.snippet.len() > 20 {
        confidence += 0.1;
    }
    confidence = confidence.min(1.0);

    // 从标题或 URL 提取技能名
    let name = extract_name(&hit.title, &hit.url);

    Some(json!({
        "name": name,
        "description": hit.snippet.clone(),
        "source": format!("{:?}", hit.source),
        "url": hit.url.clone(),
        "confidence": (confidence * 100.0).round() / 100.0,
    }))
}

/// 从标题或 URL 中提取技能名称
fn extract_name(title: &str, url: &str) -> String {
    // 尝试从标题提取：移除 "skill"、"plugin" 等后缀
    let cleaned = title
        .split("—")
        .next()
        .unwrap_or(title)
        .split("–")
        .next()
        .unwrap_or(title)
        .trim();
    if cleaned.len() < 50 && !cleaned.is_empty() {
        return cleaned.to_string();
    }
    // fallback：从 GitHub URL 提取仓库名
    if url.contains("github.com/") {
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() >= 5 {
            return format!("{}/{}", parts[3], parts[4]);
        }
    }
    // 最终 fallback：截断标题
    title.chars().take(40).collect()
}
