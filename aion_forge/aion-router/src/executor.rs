use std::{
    fs::{self, OpenOptions},
    io::Write,
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::security::{AiSecurityReviewer, Security, Verdict};
use aion_intel::immunity::ImmunitySystem;
use aion_intel::discovery_radar::DiscoveryRadar;
use aion_types::types::{ExecutionContext, ExecutionResponse, RouterPaths, SkillDefinition};
use aion_memory::memory::{MemoryCategory, MemoryManager};
use aion_memory::memory_distiller::MemoryDistiller;

pub struct Executor;

impl Executor {
    pub fn validate_permissions(skill: &SkillDefinition, paths: &RouterPaths) -> Result<()> {
        Security::validate(skill, paths)
    }

    pub fn execute(
        skill: &SkillDefinition,
        context: &ExecutionContext,
        paths: &RouterPaths,
    ) -> Result<ExecutionResponse> {
        Self::validate_permissions(skill, paths)?;
        paths.ensure_base_dirs()?;

        if let Verdict::Deny(reason) =
            AiSecurityReviewer::review_pre_execution(skill, context, paths)
        {
            return Err(anyhow!("security review blocked execution: {}", reason));
        }

        // Immunity Pre-check & Sanitization
        let mut sanitized_task = context.task.clone();
        ImmunitySystem::sanitize_instruction(&mut sanitized_task);
        ImmunitySystem::pre_check_command(&sanitized_task)?;

        let response = if skill.metadata.entrypoint.starts_with("builtin:") {
            Self::execute_builtin(skill, context)
        } else {
            Err(anyhow!(
                "external entrypoints are not supported. Got: {}",
                skill.metadata.entrypoint
            ))
        }?;

        if let Verdict::Deny(reason) =
            AiSecurityReviewer::review_post_execution(skill, &response, paths)
        {
            return Err(anyhow!("security review blocked output: {}", reason));
        }

        Self::append_log(skill, context, &response, paths)?;
        Ok(response)
    }

    fn execute_builtin(skill: &SkillDefinition, context: &ExecutionContext) -> Result<ExecutionResponse> {
        let builtin = skill.metadata.entrypoint.trim_start_matches("builtin:");
        let result = match builtin {
            "yaml_parse"      => Self::run_yaml_parse(context),
            "json_parse"      => Self::run_json_parse(context),
            "toml_parse"      => Self::run_toml_parse(context),
            "csv_parse"       => Self::run_csv_parse(context),
            "text_summarize"  => Self::run_ai_task(context, "Summarize the following text concisely:"),
            "text_translate"  => Self::run_ai_task(context, "Translate the following text. Detect source language automatically:"),
            "text_classify"   => Self::run_ai_task(context, "Classify the following text into a category. Return only the category name:"),
            "text_extract"    => Self::run_ai_task(context, "Extract the key information and entities from the following text as JSON:"),
            "text_diff"       => Self::run_text_diff(context),
            "text_embed"      => Self::run_text_embed(context),
            "web_search"      => Self::run_web_search(context),
            "http_fetch"      => Self::run_http_fetch(context),
            "code_generate"   => Self::run_ai_task(context, "Generate Rust code for the following requirement. Return only the code:"),
            "code_test"       => Self::run_ai_task(context, "Write Rust unit tests for the following code:"),
            "code_lint"       => Self::run_ai_task(context, "Review the following Rust code for issues and suggest fixes:"),
            "image_describe"  => Self::run_ai_task(context, "Describe the image at the given path or URL:"),
            "markdown_render" => Self::run_markdown_render(context),
            "pdf_parse"       => Self::run_ai_task(context, "Extract and structure the text content from this PDF path:"),
            "memory_remember" => Self::run_memory_remember(context),
            "memory_recall"   => Self::run_memory_recall(context),
            "memory_distill"  => Self::run_memory_distill(context),
            "discovery_search" => Self::run_discovery_search(context),
            "shell_exec"      => Err(anyhow!("shell_exec is disabled for security reasons")),
            "echo" | "placeholder" => Ok(json!({
                "task": context.task, "capability": context.capability,
                "skill": skill.metadata.name,
                "notice": "placeholder -- no real implementation for this capability yet",
            })),
            other => Err(anyhow!("unknown builtin: {other}")),
        }?;
        Ok(ExecutionResponse { status: "ok".to_string(), result, artifacts: Value::Object(Default::default()), error: None })
    }

    fn run_ai_task(ctx: &ExecutionContext, instruction: &str) -> Result<Value> {
        let text = ctx.context["text"].as_str().or_else(|| ctx.context["input"].as_str()).unwrap_or(&ctx.task).to_string();
        let base_url = std::env::var("AI_BASE_URL").unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
        let api_key  = std::env::var("AI_API_KEY").unwrap_or_else(|_| "ollama".to_string());
        let model    = std::env::var("AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b".to_string());
        let body = json!({"model": model, "messages": [{"role":"system","content":instruction},{"role":"user","content":text}], "temperature": 0.3});
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(30)).build()?;
        let resp: Value = client.post(format!("{}/chat/completions", base_url)).header("Authorization", format!("Bearer {}", api_key)).json(&body).send()?.json()?;
        let content = resp["choices"][0]["message"]["content"].as_str().or_else(|| resp["result"].as_str()).unwrap_or("").to_string();
        Ok(json!({"task": ctx.task, "capability": ctx.capability, "output": content}))
    }

    fn run_yaml_parse(ctx: &ExecutionContext) -> Result<Value> {
        let text = ctx.context["text"].as_str().or_else(|| ctx.context["input"].as_str()).unwrap_or(&ctx.task).to_string();
        match Self::parse_yaml_naive(&text) {
            Ok(p)  => Ok(json!({"parsed": p, "format": "yaml"})),
            Err(e) => Ok(json!({"error": e.to_string(), "raw": text, "format": "yaml"})),
        }
    }

    fn parse_yaml_naive(text: &str) -> Result<Value> {
        let mut root = serde_json::Map::new();
        let mut current_key: Option<String> = None;
        let mut list_buf: Vec<Value> = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') { continue; }
            if let Some(stripped) = t.strip_prefix("- ") {
                list_buf.push(Self::yaml_scalar(stripped));
                continue;
            }
            if !list_buf.is_empty() {
                if let Some(k) = current_key.take() { root.insert(k, Value::Array(std::mem::take(&mut list_buf))); }
            }
            if let Some(pos) = t.find(": ") {
                let key = t[..pos].trim().to_string();
                let val = t[pos+2..].trim();
                if val.is_empty() { current_key = Some(key); } else { root.insert(key, Self::yaml_scalar(val)); }
            } else if t.ends_with(':') {
                current_key = Some(t.trim_end_matches(':').to_string());
            }
        }
        if !list_buf.is_empty() { if let Some(k) = current_key { root.insert(k, Value::Array(list_buf)); } }
        if root.is_empty() { anyhow::bail!("no key-value pairs found in YAML"); }
        Ok(Value::Object(root))
    }

    fn yaml_scalar(s: &str) -> Value {
        let s = s.trim().trim_matches('"').trim_matches('\'');
        match s {
            "null"|"~" => Value::Null, "true" => Value::Bool(true), "false" => Value::Bool(false),
            _ => {
                if let Ok(n) = s.parse::<i64>() {
                    json!(n)
                } else if let Ok(f) = s.parse::<f64>() {
                    json!(f)
                } else {
                    Value::String(s.to_string())
                }
            }
        }
    }

    fn run_json_parse(ctx: &ExecutionContext) -> Result<Value> {
        let text = ctx.context["text"].as_str().or_else(|| ctx.context["input"].as_str()).unwrap_or(&ctx.task).to_string();
        match serde_json::from_str::<Value>(&text) {
            Ok(p)  => Ok(json!({"parsed": p, "format": "json"})),
            Err(e) => Ok(json!({"error": e.to_string(), "raw": text, "format": "json"})),
        }
    }

    fn run_toml_parse(ctx: &ExecutionContext) -> Result<Value> {
        let text = ctx.context["text"].as_str().or_else(|| ctx.context["input"].as_str()).unwrap_or(&ctx.task).to_string();
        let mut root = serde_json::Map::new();
        let mut section = String::new();
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() || t.starts_with('#') { continue; }
            if t.starts_with('[') && t.ends_with(']') {
                section = t[1..t.len()-1].to_string();
                root.entry(section.clone()).or_insert_with(|| Value::Object(serde_json::Map::new()));
                continue;
            }
            if let Some(eq) = t.find(" = ") {
                let key = t[..eq].trim().to_string();
                let val = Self::yaml_scalar(t[eq+3..].trim());
                if section.is_empty() { root.insert(key, val); }
                else if let Some(Value::Object(sec)) = root.get_mut(&section) { sec.insert(key, val); }
            }
        }
        Ok(json!({"parsed": Value::Object(root), "format": "toml"}))
    }

    fn run_csv_parse(ctx: &ExecutionContext) -> Result<Value> {
        let text = ctx.context["text"].as_str().or_else(|| ctx.context["input"].as_str()).unwrap_or(&ctx.task).to_string();
        let mut lines = text.lines();
        let headers: Vec<String> = lines.next().unwrap_or("").split(',').map(|h| h.trim().to_string()).collect();
        let rows: Vec<Value> = lines.filter(|l| !l.trim().is_empty()).map(|line| {
            let cells: Vec<&str> = line.split(',').collect();
            let obj: serde_json::Map<String,Value> = headers.iter().enumerate()
                .map(|(i,h)| (h.clone(), Self::yaml_scalar(cells.get(i).map(|s| s.trim()).unwrap_or(""))))
                .collect();
            Value::Object(obj)
        }).collect();
        let count = rows.len();
        Ok(json!({"headers": headers, "rows": rows, "count": count, "format": "csv"}))
    }

    fn run_text_diff(ctx: &ExecutionContext) -> Result<Value> {
        let a = ctx.context["a"].as_str().unwrap_or("").to_string();
        let b = ctx.context["b"].as_str().unwrap_or("").to_string();
        let al: Vec<&str> = a.lines().collect();
        let bl: Vec<&str> = b.lines().collect();
        let mut diff: Vec<Value> = Vec::new();
        let mut added = 0usize; let mut removed = 0usize;
        for l in &al { if !bl.contains(l) { removed += 1; diff.push(json!({"op":"-","line":l})); } }
        for l in &bl { if !al.contains(l) { added   += 1; diff.push(json!({"op":"+","line":l})); } }
        Ok(json!({"added": added, "removed": removed, "diff": diff}))
    }

    fn run_text_embed(ctx: &ExecutionContext) -> Result<Value> {
        let text = Self::require_text(ctx)?;
        let mut freq: std::collections::BTreeMap<String,usize> = std::collections::BTreeMap::new();
        for word in text.split_whitespace() {
            let w = word.to_ascii_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string();
            if w.len() > 2 { *freq.entry(w).or_insert(0) += 1; }
        }
        let total: usize = freq.values().sum();
        let terms = freq.len();
        let vector: serde_json::Map<String,Value> = freq.into_iter().map(|(k,v)| (k, json!(v as f64 / total.max(1) as f64))).collect();
        Ok(json!({"method":"tf_bow","vector":vector,"terms":terms}))
    }

    fn run_markdown_render(ctx: &ExecutionContext) -> Result<Value> {
        let text = Self::require_text(ctx)?;
        let mut sections: Vec<Value> = Vec::new();
        let mut heading = String::new();
        let mut body: Vec<String> = Vec::new();
        for line in text.lines() {
            if let Some(stripped) = line.strip_prefix("# ") {
                if !heading.is_empty() { sections.push(json!({"heading": heading, "body": body.join("\n")})); body.clear(); }
                heading = stripped.to_string();
            } else if let Some(stripped) = line.strip_prefix("## ") {
                body.push(format!("[{}]", stripped));
            } else {
                body.push(line.to_string());
            }
        }
        if !heading.is_empty() { sections.push(json!({"heading": heading, "body": body.join("\n")})); }
        Ok(json!({"sections": sections, "format": "markdown"}))
    }

    fn run_web_search(ctx: &ExecutionContext) -> Result<Value> {
        let query = ctx.context["query"].as_str().unwrap_or(&ctx.task).to_string();
        let key = std::env::var("SERPAPI_KEY").unwrap_or_default();
        if key.is_empty() { return Ok(json!({"notice":"SERPAPI_KEY not configured","query":query,"results":[]})); }
        let url = format!("https://serpapi.com/search.json?q={}&api_key={}&num=5", urlencoding_simple(&query), key);
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        let resp: Value = client.get(&url).send()?.json()?;
        Ok(json!({"query": query, "results": resp["organic_results"]}))
    }

    fn run_http_fetch(ctx: &ExecutionContext) -> Result<Value> {
        let url = ctx.context["url"].as_str().ok_or_else(|| anyhow!("http_fetch requires context.url"))?.to_string();
        if !url.starts_with("https://") { anyhow::bail!("http_fetch only allows HTTPS URLs"); }
        let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        let resp = client.get(&url).send()?;
        let status = resp.status().as_u16();
        let body = resp.text()?;
        Ok(json!({"url": url, "status": status, "body": body}))
    }

    fn require_text(ctx: &ExecutionContext) -> Result<String> {
        ctx.context["text"].as_str().or_else(|| ctx.context["input"].as_str())
            .map(str::to_string).ok_or_else(|| anyhow!("context.text is required for this skill"))
    }

    fn run_memory_remember(ctx: &ExecutionContext) -> Result<Value> {
        let content = ctx.context["content"].as_str()
            .or_else(|| ctx.context["text"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();
        let category_str = ctx.context["category"].as_str().unwrap_or("decision");
        let importance = ctx.context["importance"].as_u64().unwrap_or(5) as u8;
        let session = ctx.context["session"].as_str().unwrap_or("unknown").to_string();

        let category = match category_str {
            "lesson"       => MemoryCategory::Lesson,
            "error"        => MemoryCategory::Error,
            "preference"   => MemoryCategory::Preference,
            "architecture" => MemoryCategory::Architecture,
            "progress"     => MemoryCategory::TaskProgress,
            _              => MemoryCategory::Decision,
        };

        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let id = manager.remember(category, &content, &session, importance)?;

        Ok(json!({
            "status": "remembered",
            "memory_id": id,
            "content": content,
        }))
    }

    fn run_memory_recall(ctx: &ExecutionContext) -> Result<Value> {
        let query = ctx.context["query"].as_str()
            .or_else(|| ctx.context["text"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();
        let limit = ctx.context["limit"].as_u64().unwrap_or(5) as usize;

        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let results = manager.recall(&query, limit)?;

        let entries: Vec<Value> = results.iter().map(|e| json!({
            "id": e.id,
            "content": e.content,
            "category": format!("{:?}", e.category),
            "importance": e.importance,
            "access_count": e.access_count,
        })).collect();

        Ok(json!({
            "query": query,
            "results_count": entries.len(),
            "results": entries,
        }))
    }

    fn run_memory_distill(ctx: &ExecutionContext) -> Result<Value> {
        let max_entries = ctx.context["max_entries"].as_u64().unwrap_or(200) as usize;

        let workspace = std::env::current_dir().unwrap_or_default();
        let manager = MemoryManager::new(&workspace);
        let report = MemoryDistiller::distill(&manager, max_entries)?;

        // Also regenerate CONTEXT.md after distillation
        let _ = manager.generate_context_md();

        Ok(report.to_json())
    }

    fn append_log(skill: &SkillDefinition, context: &ExecutionContext, response: &ExecutionResponse, paths: &RouterPaths) -> Result<()> {
        if let Some(parent) = paths.executions_log.parent() { fs::create_dir_all(parent)?; }
        let mut file = OpenOptions::new().create(true).append(true).open(&paths.executions_log)?;
        let line = json!({"timestamp": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(), "skill": skill.metadata.name, "capability": context.capability, "status": response.status});
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
        Ok(())
    }

    fn run_discovery_search(ctx: &ExecutionContext) -> Result<Value> {
        let query = ctx.context["query"].as_str()
            .or_else(|| ctx.context["text"].as_str())
            .unwrap_or(&ctx.task)
            .to_string();
        // Build minimal paths for local search; cascade search will gracefully degrade
        let workspace = std::env::current_dir().unwrap_or_default();
        let paths = RouterPaths::for_workspace(&workspace);
        let result = DiscoveryRadar::cascade_search(&query, &paths)?;
        Ok(DiscoveryRadar::to_json(&result))
    }
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'|b'a'..=b'z'|b'0'..=b'9'|b'-'|b'_'|b'.'|b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}


