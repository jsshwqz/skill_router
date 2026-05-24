//! GitHub 趋势监控 builtin：github_trending
//!
//! 查询 GitHub 热门仓库，按日/周/月/总榜分组，自动过滤 AI 相关项目。
//! 支持历史追踪（通过 learner 事件日志），便于对比趋势变化。
//!
//! 数据源：GitHub Search API（无需 token，公共限流 10 次/分钟）

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};
use super::BuiltinSkill;

/// AI 相关关键词，用于过滤和标注仓库关联度
const AI_KEYWORDS: &[(&str, &str)] = &[
    ("ai", "AI"),
    ("agent", "AI Agent"),
    ("claude", "Claude/Anthropic"),
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("gpt", "GPT"),
    ("llm", "LLM"),
    ("mcp", "Model Context Protocol"),
    ("skill", "Skill/Plugin"),
    ("coding", "Coding Assistant"),
    ("automation", "Automation"),
    ("pipeline", "Pipeline"),
    ("rag", "RAG"),
    ("embedding", "Embedding"),
    ("prompt", "Prompt Engineering"),
    ("function-call", "Function Calling"),
    ("tool-use", "Tool Use"),
    ("codex", "Codex"),
    ("copilot", "Copilot"),
    ("rust", "Rust"),
];

/// 时间周期
#[derive(Debug, Clone, Copy, PartialEq)]
enum Period {
    Daily,
    Weekly,
    Monthly,
    AllTime,
}

impl Period {
    fn as_str(&self) -> &'static str {
        match self {
            Period::Daily => "daily",
            Period::Weekly => "weekly",
            Period::Monthly => "monthly",
            Period::AllTime => "all_time",
        }
    }

    fn date_filter(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = match self {
            Period::Daily => 1,
            Period::Weekly => 7,
            Period::Monthly => 30,
            Period::AllTime => 365 * 5, // 5 years ≈ all relevant
        };
        let secs_per_day = 86400;
        let cutoff = now.saturating_sub(days * secs_per_day);
        // Convert to YYYY-MM-DD
        let days_since_epoch = cutoff / secs_per_day;
        let date = date_from_days(days_since_epoch as i64);
        if self == &Period::AllTime {
            String::new() // no date filter for all-time
        } else {
            format!("created:>{}", date)
        }
    }
}

/// 将 epoch days 转换为 YYYY-MM-DD 字符串
fn date_from_days(days: i64) -> String {
    // 从 1970-01-01 开始计算
    let mut y = 1970i64;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let months = [31, if is_leap(y) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for &days_in_month in &months {
        if remaining < days_in_month {
            break;
        }
        remaining -= days_in_month;
        m += 1;
    }
    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

pub struct GithubTrending;

#[async_trait::async_trait]
impl BuiltinSkill for GithubTrending {
    fn name(&self) -> &'static str {
        "github_trending"
    }

    async fn execute(&self, _skill: &SkillDefinition, ctx: &ExecutionContext) -> Result<Value> {
        let period_str = ctx.context["period"]
            .as_str()
            .unwrap_or("weekly");
        let limit = ctx.context["limit"]
            .as_u64()
            .unwrap_or(10)
            .min(25) as usize;
        let min_stars = ctx.context["min_stars"]
            .as_u64()
            .unwrap_or(50);

        let period = match period_str {
            "daily" => Period::Daily,
            "weekly" => Period::Weekly,
            "monthly" => Period::Monthly,
            "all_time" | "all" => Period::AllTime,
            _ => Period::Weekly, // default
        };

        let date_filter = period.date_filter();
        let additional = if period == Period::AllTime {
            format!("stars:>{}", min_stars)
        } else {
            format!("stars:>{}", min_stars)
        };

        let query = if date_filter.is_empty() {
            format!("q={}&sort=stars&order=desc&per_page={}", additional, limit)
        } else {
            format!("q={}+{}&sort=stars&order=desc&per_page={}", date_filter, additional, limit)
        };

        let url = format!("https://api.github.com/search/repositories?{}", query);
        let client = reqwest::Client::builder()
            .user_agent("aion-forge/1.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let resp = client.get(&url).send().await.map_err(|e| {
            anyhow!("GitHub API request failed: {}", e)
        })?;
        let status = resp.status();
        let body: Value = resp.json().await.map_err(|e| {
            anyhow!("Failed to parse GitHub API response: {}", e)
        })?;

        if !status.is_success() {
            let msg = body["message"].as_str().unwrap_or("unknown error");
            return Ok(json!({
                "error": format!("GitHub API error ({}): {}", status, msg),
                "period": period.as_str(),
            }));
        }

        let items = body["items"].as_array().ok_or_else(|| {
            anyhow!("GitHub API returned unexpected format")
        })?;

        let mut repos: Vec<Value> = Vec::new();
        let mut ai_count = 0;

        for item in items {
            let name = item["full_name"].as_str().unwrap_or("unknown");
            let desc = item["description"].as_str().unwrap_or("").to_string();
            let stars = item["stargazers_count"].as_u64().unwrap_or(0);
            let url = item["html_url"].as_str().unwrap_or("");
            let lang = item["language"].as_str().unwrap_or("N/A");
            let topics: Vec<&str> = item["topics"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let created = item["created_at"].as_str().unwrap_or("");
            let pushed = item["pushed_at"].as_str().unwrap_or("");

            // 检测 AI 相关度
            let combined = format!("{} {} {} {:?}", name.to_lowercase(), desc.to_lowercase(), lang.to_lowercase(), topics);
            let mut matched_keywords: Vec<&str> = Vec::new();
            let mut tag = String::new();

            for (kw, label) in AI_KEYWORDS {
                if combined.contains(kw) {
                    matched_keywords.push(kw);
                    if tag.is_empty() {
                        tag = label.to_string();
                    }
                }
            }

            let is_ai_related = !matched_keywords.is_empty();
            if is_ai_related {
                ai_count += 1;
            }

            repos.push(json!({
                "name": name,
                "description": if desc.len() > 120 { format!("{}...", &desc[..120]) } else { desc },
                "stars": stars,
                "url": url,
                "language": lang,
                "topics": topics,
                "created_at": created,
                "pushed_at": pushed,
                "ai_related": is_ai_related,
                "ai_tag": tag,
                "matched_keywords": matched_keywords,
            }));
        }

        // 按星数排（API 已排序，但以防万一）
        repos.sort_by(|a, b| b["stars"].as_u64().unwrap_or(0).cmp(&a["stars"].as_u64().unwrap_or(0)));

        // 分离 AI 相关和非 AI 相关
        let ai_repos: Vec<&Value> = repos.iter().filter(|r| r["ai_related"].as_bool().unwrap_or(false)).collect();
        let other_repos: Vec<&Value> = repos.iter().filter(|r| !r["ai_related"].as_bool().unwrap_or(false)).collect();

        // 简单摘要
        let summary = if ai_repos.is_empty() {
            format!("{} 期 GitHub 趋势中未发现 AI 相关项目（共 {} 个热门仓库）", period.as_str(), repos.len())
        } else {
            format!(
                "{} 期发现 {} 个 AI 相关项目（共 {} 个，占比 {:.0}%）。热门：{}",
                period.as_str(),
                ai_repos.len(),
                repos.len(),
                ai_repos.len() as f64 / repos.len().max(1) as f64 * 100.0,
                ai_repos.iter().take(3).map(|r| r["name"].as_str().unwrap_or("")).collect::<Vec<_>>().join(", "),
            )
        };

        Ok(json!({
            "period": period.as_str(),
            "total_repos": repos.len(),
            "ai_related_count": ai_count,
            "summary": summary,
            "ai_related": ai_repos.iter().map(|r| (*r).clone()).collect::<Vec<_>>(),
            "others": other_repos.iter().map(|r| (*r).clone()).collect::<Vec<_>>(),
        }))
    }
}
