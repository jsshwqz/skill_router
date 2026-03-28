//! 网络类 builtin 技能：web_search, http_fetch, discovery_search

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_intel::discovery_radar::DiscoveryRadar;
use aion_types::types::{ExecutionContext, RouterPaths, SkillDefinition};

use super::{urlencoding_simple, BuiltinSkill};

// ── web_search ──────────────────────────────────────────────────────────────

pub struct WebSearch;

#[async_trait::async_trait]
impl BuiltinSkill for WebSearch {
    fn name(&self) -> &'static str { "web_search" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let query = context.context["query"]
            .as_str()
            .unwrap_or(&context.task)
            .to_string();
        let key = std::env::var("SERPAPI_KEY").unwrap_or_default();
        if key.is_empty() {
            return Ok(
                json!({"notice": "SERPAPI_KEY not configured", "query": query, "results": []}),
            );
        }
        let url = format!(
            "https://serpapi.com/search.json?q={}&api_key={}&num=5",
            urlencoding_simple(&query),
            key
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let resp: Value = client.get(&url).send().await?.json().await?;
        Ok(json!({"query": query, "results": resp["organic_results"]}))
    }
}

// ── http_fetch ──────────────────────────────────────────────────────────────

pub struct HttpFetch;

#[async_trait::async_trait]
impl BuiltinSkill for HttpFetch {
    fn name(&self) -> &'static str { "http_fetch" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let url = context.context["url"]
            .as_str()
            .ok_or_else(|| anyhow!("http_fetch requires context.url"))?
            .to_string();
        if !url.starts_with("https://") {
            anyhow::bail!("http_fetch only allows HTTPS URLs");
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let resp = client.get(&url).send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await?;
        Ok(json!({"url": url, "status": status, "body": body}))
    }
}

// ── discovery_search ────────────────────────────────────────────────────────

pub struct DiscoverySearch;

#[async_trait::async_trait]
impl BuiltinSkill for DiscoverySearch {
    fn name(&self) -> &'static str { "discovery_search" }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let query = context.context["query"]
            .as_str()
            .or_else(|| context.context["text"].as_str())
            .unwrap_or(&context.task)
            .to_string();
        let workspace = std::env::current_dir().unwrap_or_default();
        let paths = RouterPaths::for_workspace(&workspace);
        let result = DiscoveryRadar::cascade_search(&query, &paths).await?;
        Ok(DiscoveryRadar::to_json(&result))
    }
}
