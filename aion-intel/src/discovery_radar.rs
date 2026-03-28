use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::online_search::TrustedSourceSearch;
use aion_types::types::RouterPaths;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: SearchSource,
    pub relevance_score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSource {
    GoogleApi,
    HttpDirect,
    LocalTrusted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub sources_tried: Vec<String>,
    pub sources_succeeded: Vec<String>,
}

// ---------------------------------------------------------------------------
// DiscoveryRadar — three-tier cascading search
// ---------------------------------------------------------------------------

pub struct DiscoveryRadar;

impl DiscoveryRadar {
    /// Entry-point: cascade Google → HTTP fallback → local trusted sources.
    /// Every tier is independent — failure in one tier does not block the others.
    pub async fn cascade_search(query: &str, paths: &RouterPaths) -> Result<DiscoveryResult> {
        let mut all_hits: Vec<SearchHit> = Vec::new();
        let mut sources_tried: Vec<String> = Vec::new();
        let mut sources_succeeded: Vec<String> = Vec::new();

        // --- Tier 1: SerpAPI (Google) ---
        sources_tried.push("google_api".into());
        match Self::search_google(query).await {
            Ok(hits) if !hits.is_empty() => {
                sources_succeeded.push("google_api".into());
                all_hits.extend(hits);
            }
            _ => {} // graceful degradation
        }

        // --- Tier 2: HTTP direct scraping (DuckDuckGo Lite) ---
        sources_tried.push("http_direct".into());
        match Self::search_http_fallback(query).await {
            Ok(hits) if !hits.is_empty() => {
                sources_succeeded.push("http_direct".into());
                all_hits.extend(hits);
            }
            _ => {}
        }

        // --- Tier 3: Local trusted sources ---
        sources_tried.push("local_trusted".into());
        match Self::search_local(query, paths).await {
            Ok(hits) if !hits.is_empty() => {
                sources_succeeded.push("local_trusted".into());
                all_hits.extend(hits);
            }
            _ => {}
        }

        let hits = Self::deduplicate_and_rank(all_hits);
        let hits = Self::filter_noise(hits);

        Ok(DiscoveryResult {
            query: query.to_string(),
            hits,
            sources_tried,
            sources_succeeded,
        })
    }

    // -----------------------------------------------------------------------
    // Tier implementations
    // -----------------------------------------------------------------------

    async fn search_google(query: &str) -> Result<Vec<SearchHit>> {
        let key = std::env::var("SERPAPI_KEY").unwrap_or_default();
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "https://serpapi.com/search.json?q={}&api_key={}&num=5",
            urlencoding_simple(query),
            key
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let resp: Value = client.get(&url).send().await?.json().await?;
        let hits = resp["organic_results"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .enumerate()
            .map(|(i, r)| SearchHit {
                title: r["title"].as_str().unwrap_or("").to_string(),
                url: r["link"].as_str().unwrap_or("").to_string(),
                snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                source: SearchSource::GoogleApi,
                relevance_score: 1.0 - (i as f64 * 0.1),
            })
            .collect();
        Ok(hits)
    }

    async fn search_http_fallback(query: &str) -> Result<Vec<SearchHit>> {
        // DuckDuckGo Instant Answer JSON API (no key required)
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding_simple(query)
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()?;
        let resp: Value = client.get(&url).send().await?.json().await?;

        let mut hits = Vec::new();

        // Abstract result
        if let Some(abstract_text) = resp["AbstractText"].as_str() {
            if !abstract_text.is_empty() {
                hits.push(SearchHit {
                    title: resp["Heading"].as_str().unwrap_or("DuckDuckGo Result").to_string(),
                    url: resp["AbstractURL"].as_str().unwrap_or("").to_string(),
                    snippet: abstract_text.chars().take(300).collect(),
                    source: SearchSource::HttpDirect,
                    relevance_score: 0.8,
                });
            }
        }

        // Related topics
        if let Some(topics) = resp["RelatedTopics"].as_array() {
            for (i, topic) in topics.iter().take(4).enumerate() {
                if let Some(text) = topic["Text"].as_str() {
                    hits.push(SearchHit {
                        title: text.chars().take(80).collect(),
                        url: topic["FirstURL"].as_str().unwrap_or("").to_string(),
                        snippet: text.to_string(),
                        source: SearchSource::HttpDirect,
                        relevance_score: 0.7 - (i as f64 * 0.05),
                    });
                }
            }
        }

        Ok(hits)
    }

    async fn search_local(query: &str, paths: &RouterPaths) -> Result<Vec<SearchHit>> {
        // Reuse existing TrustedSourceSearch to find matching local skills
        let q_lower = query.to_ascii_lowercase();
        let keywords: Vec<&str> = q_lower.split_whitespace().collect();

        let skills = TrustedSourceSearch::search(paths, keywords.first().unwrap_or(&""))?;
        let hits: Vec<SearchHit> = skills
            .into_iter()
            .map(|skill| {
                let cap_match = skill
                    .metadata
                    .capabilities
                    .iter()
                    .any(|c| keywords.iter().any(|kw| c.contains(kw)));
                SearchHit {
                    title: format!("Local Skill: {}", skill.metadata.name),
                    url: format!("local://{}", skill.metadata.name),
                    snippet: format!(
                        "Capabilities: {}",
                        skill.metadata.capabilities.join(", ")
                    ),
                    source: SearchSource::LocalTrusted,
                    relevance_score: if cap_match { 0.9 } else { 0.5 },
                }
            })
            .collect();
        Ok(hits)
    }

    // -----------------------------------------------------------------------
    // Post-processing
    // -----------------------------------------------------------------------

    /// Deduplicate by URL and sort by relevance_score descending.
    pub fn deduplicate_and_rank(results: Vec<SearchHit>) -> Vec<SearchHit> {
        let mut seen = HashSet::new();
        let mut deduped: Vec<SearchHit> = results
            .into_iter()
            .filter(|hit| {
                if hit.url.is_empty() {
                    return true; // keep hits without URL
                }
                seen.insert(hit.url.clone())
            })
            .collect();
        deduped.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        deduped
    }

    /// Filter out low-quality / noisy results.
    pub fn filter_noise(hits: Vec<SearchHit>) -> Vec<SearchHit> {
        hits.into_iter()
            .filter(|hit| {
                if hit.title.is_empty() && hit.snippet.is_empty() {
                    return false;
                }
                if hit.snippet.len() < 10 && hit.source != SearchSource::LocalTrusted {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Convert full result to serde_json::Value for Executor integration.
    pub fn to_json(result: &DiscoveryResult) -> Value {
        json!({
            "query": result.query,
            "total_hits": result.hits.len(),
            "sources_tried": result.sources_tried,
            "sources_succeeded": result.sources_succeeded,
            "hits": result.hits.iter().map(|h| json!({
                "title": h.title,
                "url": h.url,
                "snippet": h.snippet,
                "source": h.source,
                "relevance_score": h.relevance_score,
            })).collect::<Vec<Value>>(),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
