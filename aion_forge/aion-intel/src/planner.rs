use std::{collections::HashMap, sync::{Mutex, OnceLock}};
use anyhow::Result;
use serde_json::json;
use aion_types::capability_registry::CapabilityRegistry;

static INFER_CACHE: OnceLock<Mutex<InferCache>> = OnceLock::new();
struct InferCache { map: HashMap<u64,String>, order: std::collections::VecDeque<u64> }
impl InferCache {
    fn new() -> Self { Self { map: HashMap::new(), order: std::collections::VecDeque::new() } }
    fn get(&self, k: u64) -> Option<&str> { self.map.get(&k).map(String::as_str) }
    fn insert(&mut self, k: u64, v: String) {
        if self.map.contains_key(&k) { return; }
        if self.map.len() >= 256 { if let Some(o) = self.order.pop_front() { self.map.remove(&o); } }
        self.map.insert(k, v); self.order.push_back(k);
    }
}
fn cache() -> &'static Mutex<InferCache> { INFER_CACHE.get_or_init(|| Mutex::new(InferCache::new())) }
fn hash_task(t: &str) -> u64 {
    use std::hash::{Hash,Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.to_ascii_lowercase().trim().hash(&mut h); h.finish()
}

pub struct Planner;
impl Planner {
    pub fn infer_capability(task: &str, registry: &CapabilityRegistry) -> Result<Option<String>> {
        let key = hash_task(task);
        if let Ok(c) = cache().lock() { if let Some(v) = c.get(key) { return Ok(Some(v.to_string())); } }
        let r = Self::infer_via_ai(task, registry).ok().flatten().or_else(|| Self::infer_via_keywords(task, registry));
        if let Some(ref cap) = r { if let Ok(mut c) = cache().lock() { c.insert(key, cap.clone()); } }
        Ok(r)
    }

    pub fn infer_capability_with_paths(task: &str, registry: &mut CapabilityRegistry, paths: &aion_types::types::RouterPaths) -> Result<Option<String>> {
        let key = hash_task(task);
        if let Ok(c) = cache().lock() { if let Some(v) = c.get(key) { return Ok(Some(v.to_string())); } }
        let r = Self::infer_via_ai(task, registry).ok().flatten().or_else(|| Self::infer_via_keywords(task, registry));
        let r = if r.is_none() { Self::ai_discover(task, registry, &paths.capabilities_dir) } else { r };
        if let Some(ref cap) = r { if let Ok(mut c) = cache().lock() { c.insert(key, cap.clone()); } }
        Ok(r)
    }

    fn ai_discover(task: &str, registry: &mut CapabilityRegistry, cap_dir: &std::path::Path) -> Option<String> {
        let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key  = std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model    = std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());
        let prompt = format!("A user wants to: \"{task}\"\nPropose a new capability name in snake_case. Return ONLY the name.");
        let body = json!({"model":model,"messages":[{"role":"user","content":prompt}],"temperature":0.0,"max_tokens":16});
        let name = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(8)).build().ok()
            .and_then(|c| c.post(format!("{}/chat/completions",base_url)).header("Authorization",format!("Bearer {}",api_key)).json(&body).send().ok())
            .and_then(|r| r.json::<serde_json::Value>().ok())
            .and_then(|v| { let raw = v["choices"][0]["message"]["content"].as_str().or_else(|| v["result"].as_str()).unwrap_or("").trim().to_ascii_lowercase(); let n: String = raw.chars().filter(|c| c.is_alphanumeric()||*c=='_').collect(); if !n.is_empty() && registry.validate_name(&n).is_ok() { Some(n) } else { None } })?;
        let _ = registry.persist_to_dir(&name, task, cap_dir);
        Some(name)
    }

    fn infer_via_ai(task: &str, registry: &CapabilityRegistry) -> Result<Option<String>> {
        let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key  = std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model    = std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());
        let caps: Vec<String> = registry.definitions().map(|d| format!("- {} : {}", d.name, d.description)).collect();
        let sys = format!("You are a capability classifier. Return ONLY the best matching capability name from this list, or 'none'.\n\nCapabilities:\n{}", caps.join("\n"));
        let body = json!({"model":model,"messages":[{"role":"system","content":sys},{"role":"user","content":task}],"temperature":0.0,"max_tokens":32});
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(8)).build()?;
        let resp: serde_json::Value = client.post(format!("{}/chat/completions",base_url)).header("Authorization",format!("Bearer {}",api_key)).json(&body).send()?.json()?;
        let content = resp["choices"][0]["message"]["content"].as_str().or_else(|| resp["result"].as_str()).unwrap_or("").trim().to_ascii_lowercase();
        let name = content.lines().next().unwrap_or("").trim().trim_matches(|c: char| !c.is_alphanumeric()&&c!='_').to_string();
        if name=="none"||name.is_empty() { return Ok(None); }
        if registry.contains(&name) { Ok(Some(name)) } else { Ok(None) }
    }

    fn infer_via_keywords(task: &str, registry: &CapabilityRegistry) -> Option<String> {
        let n = task.trim().to_ascii_lowercase();
        let rules: &[(&[&str],&str)] = &[
            (&["yaml"],"yaml_parse"),(&["json"],"json_parse"),(&["toml"],"toml_parse"),
            (&["csv","spreadsheet","excel"],"csv_parse"),(&["pdf"],"pdf_parse"),
            (&["markdown",".md"],"markdown_render"),(&["image","photo","png","jpg"],"image_describe"),
            (&["search","lookup","find online","web"],"web_search"),
            (&["summarize","summary","tldr","brief"],"text_summarize"),
            (&["translate","translation"],"text_translate"),
            (&["classify","categorize","label"],"text_classify"),
            (&["extract","parse text","pull out"],"text_extract"),
            (&["diff","compare","difference between"],"text_diff"),
            (&["embed","embedding","vector"],"text_embed"),
            (&["http","fetch url","download","request"],"http_fetch"),
            (&["code","function","implement","write a"],"code_generate"),
            (&["unit test","spec test"],"code_test"),
            (&["lint","format","style"],"code_lint"),
            (&["navigate","galaxy","space"],"space_navigation"),
            (&["echo"],"echo"),
        ];
        for (kws,cap) in rules { 
            if kws.iter().any(|kw| n.contains(kw)) && registry.contains(cap) {
                return Some(cap.to_string());
            }
        }
        registry.definitions().find(|d| {
            n.contains(&d.name.replace('_'," "))||d.description.split_whitespace().any(|w| w.len()>4&&n.contains(&w.to_ascii_lowercase()))
        }).map(|d| d.name.clone())
    }
}
