//! 网络类 builtin 技能：web_search, http_fetch, discovery_search

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_intel::discovery_radar::DiscoveryRadar;
use aion_types::types::{ExecutionContext, RouterPaths, SkillDefinition};

use super::{urlencoding_simple, BuiltinSkill};

// ── web_search ──────────────────────────────────────────────────────────────

pub struct WebSearch;

impl WebSearch {
    async fn search_serpapi(&self, query: &str) -> Result<Value> {
        let key = std::env::var("SERPAPI_KEY").unwrap_or_default();
        if key.is_empty() {
            anyhow::bail!("SERPAPI_KEY not configured");
        }
        let url = format!(
            "https://serpapi.com/search.json?q={}&api_key={}&num=5",
            urlencoding_simple(query),
            key
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let resp: Value = client.get(&url).send().await?.json().await?;
        Ok(json!({"query": query, "results": resp["organic_results"], "provider": "serpapi"}))
    }

    async fn search_bing(&self, query: &str) -> Result<Value> {
        let key = std::env::var("BING_API_KEY").unwrap_or_default();
        if key.is_empty() {
            anyhow::bail!("BING_API_KEY not configured");
        }
        let url = format!(
            "https://api.bing.microsoft.com/v7.0/search?q={}&count=5",
            urlencoding_simple(query)
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let resp: Value = client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", &key)
            .send()
            .await?
            .json()
            .await?;
        Ok(json!({"query": query, "results": resp["webPages"]["value"], "provider": "bing"}))
    }

    async fn search_ddg(&self, query: &str) -> Result<Value> {
        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding_simple(query));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("Mozilla/5.0")
            .build()?;
        let resp = client.get(&url).send().await?.text().await?;
        // Extract results from HTML using regex (simplified)
        let results: Vec<Value> = regex::Regex::new(
            r#"<a rel="[^"]*" class="result__a"[^>]*>([^<]+)</a>.*?<a class="result__snippet"[^>]*>([^<]+)</a>"#,
        )
        .ok()
        .map(|re| {
            re.captures_iter(&resp)
                .map(|cap| {
                    json!({
                        "title": cap.get(1).map(|m| m.as_str()).unwrap_or(""),
                        "snippet": cap.get(2).map(|m| m.as_str()).unwrap_or(""),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
        Ok(json!({"query": query, "results": results, "provider": "duckduckgo"}))
    }
}

#[async_trait::async_trait]
impl BuiltinSkill for WebSearch {
    fn name(&self) -> &'static str {
        "web_search"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let query = context.context["query"].as_str().unwrap_or(&context.task).to_string();
        let provider = context.context["provider"].as_str().unwrap_or("auto");

        // P3-B #20: Multi-provider fallback (SerpAPI -> Bing -> DuckDuckGo)
        match provider {
            "serpapi" => self.search_serpapi(&query).await,
            "bing" => self.search_bing(&query).await,
            "ddg" => self.search_ddg(&query).await,
            "auto" | _ => {
                // Try SerpAPI first, fallback to others on failure
                match self.search_serpapi(&query).await {
                    Ok(result) => Ok(result),
                    Err(_) => self.search_bing(&query).await,
                }
            }
        }
    }
}

// ── http_fetch ──────────────────────────────────────────────────────────────

pub struct HttpFetch;

#[async_trait::async_trait]
impl BuiltinSkill for HttpFetch {
    fn name(&self) -> &'static str {
        "http_fetch"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let url = context.context["url"]
            .as_str()
            .ok_or_else(|| anyhow!("http_fetch requires context.url"))?
            .to_string();
        if !url.starts_with("https://") && !url.starts_with("http://") {
            anyhow::bail!("http_fetch requires http:// or https:// URL");
        }

        // P3-B #21: Proxy support (HTTP/HTTPS/SOCKS5)
        let proxy_url = context.context["proxy"].as_str();
        let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15));

        let client = if let Some(proxy) = proxy_url {
            let proxy = match proxy.trim_start_matches(|c: char| !c.is_alphanumeric()) {
                p if p.starts_with("socks5://") => reqwest::Proxy::all(proxy)?,
                p if p.starts_with("http://") || p.starts_with("https://") => reqwest::Proxy::all(proxy)?,
                _ => reqwest::Proxy::all(proxy)?,
            };
            builder.proxy(proxy).build()?
        } else {
            builder.build()?
        };

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
    fn name(&self) -> &'static str {
        "discovery_search"
    }

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
