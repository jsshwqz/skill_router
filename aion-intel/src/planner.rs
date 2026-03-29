use std::{collections::HashMap, sync::{Mutex, OnceLock}};
use anyhow::Result;
use serde_json::json;
use aion_types::capability_registry::CapabilityRegistry;

/// AI endpoint for planner inference calls, with fallback chain support.
struct PlannerEndpoint {
    label: &'static str,
    base_url: String,
    api_key: String,
    model: String,
}

impl PlannerEndpoint {
    /// Build the provider list: primary (from env) → local Ollama fallback.
    fn from_env() -> Vec<Self> {
        let mut eps = Vec::new();
        let url = std::env::var("AI_BASE_URL").unwrap_or_default();
        let key = std::env::var("AI_API_KEY").unwrap_or_default();
        let model = std::env::var("AI_MODEL").unwrap_or_default();
        if !url.is_empty() && !key.is_empty() && !model.is_empty() {
            eps.push(Self { label: "primary", base_url: url, api_key: key, model });
        }
        eps.push(Self {
            label: "ollama-local",
            base_url: "http://localhost:11434/v1".into(),
            api_key: "ollama".into(),
            model: "qwen2.5:7b".into(),
        });
        eps
    }
}

/// Send a chat completion request, trying each endpoint in order.
async fn ai_chat(body_fn: impl Fn(&str) -> serde_json::Value) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()?;
    for ep in PlannerEndpoint::from_env() {
        let resp = client
            .post(format!("{}/chat/completions", ep.base_url))
            .header("Authorization", format!("Bearer {}", ep.api_key))
            .json(&body_fn(&ep.model))
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => {
                let v: serde_json::Value = r.json().await.ok()?;
                let content = v["choices"][0]["message"]["content"]
                    .as_str()
                    .or_else(|| v["result"].as_str())
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();
                if !content.is_empty() {
                    return Some(content);
                }
            }
            _ => continue,
        }
    }
    None
}

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
    /// 纯同步的关键词推断（不调用 AI），可在持有 MutexGuard 时安全调用
    pub fn infer_via_keywords_only(task: &str, registry: &CapabilityRegistry) -> Option<String> {
        let key = hash_task(task);
        if let Ok(c) = cache().lock() { if let Some(v) = c.get(key) { return Some(v.to_string()); } }
        Self::infer_via_keywords(task, registry)
    }

    pub async fn infer_capability(task: &str, registry: &CapabilityRegistry) -> Result<Option<String>> {
        let key = hash_task(task);
        if let Ok(c) = cache().lock() { if let Some(v) = c.get(key) { return Ok(Some(v.to_string())); } }
        let r = Self::infer_via_ai(task, registry).await.ok().flatten().or_else(|| Self::infer_via_keywords(task, registry));
        if let Some(ref cap) = r { if let Ok(mut c) = cache().lock() { c.insert(key, cap.clone()); } }
        Ok(r)
    }

    pub async fn infer_capability_with_paths(task: &str, registry: &mut CapabilityRegistry, paths: &aion_types::types::RouterPaths) -> Result<Option<String>> {
        let key = hash_task(task);
        if let Ok(c) = cache().lock() { if let Some(v) = c.get(key) { return Ok(Some(v.to_string())); } }
        let r = Self::infer_via_ai(task, registry).await.ok().flatten().or_else(|| Self::infer_via_keywords(task, registry));
        let r = if r.is_none() { Self::ai_discover(task, registry, &paths.capabilities_dir).await } else { r };
        if let Some(ref cap) = r { if let Ok(mut c) = cache().lock() { c.insert(key, cap.clone()); } }
        Ok(r)
    }

    async fn ai_discover(task: &str, registry: &mut CapabilityRegistry, cap_dir: &std::path::Path) -> Option<String> {
        let prompt = format!("A user wants to: \"{task}\"\nPropose a new capability name in snake_case. Return ONLY the name.");
        let raw = ai_chat(|model| json!({"model":model,"messages":[{"role":"user","content":prompt}],"temperature":0.0,"max_tokens":16})).await?;
        let n: String = raw.chars().filter(|c| c.is_alphanumeric()||*c=='_').collect();
        if n.is_empty() || registry.validate_name(&n).is_err() { return None; }
        let _ = registry.persist_to_dir(&n, task, cap_dir);
        Some(n)
    }

    async fn infer_via_ai(task: &str, registry: &CapabilityRegistry) -> Result<Option<String>> {
        // Passthrough 模式下跳过 AI 推断，仅用关键词匹配
        if std::env::var("AI_PASSTHROUGH").map(|v| v == "true" || v == "1").unwrap_or(false) {
            return Ok(None);
        }
        let caps: Vec<String> = registry.definitions().map(|d| format!("- {} : {}", d.name, d.description)).collect();
        let sys = format!("You are a capability classifier. Return ONLY the best matching capability name from this list, or 'none'.\n\nCapabilities:\n{}", caps.join("\n"));
        let content = ai_chat(|model| json!({"model":model,"messages":[{"role":"system","content":sys},{"role":"user","content":task}],"temperature":0.0,"max_tokens":32})).await.unwrap_or_default();
        let name = content.lines().next().unwrap_or("").trim().trim_matches(|c: char| !c.is_alphanumeric()&&c!='_').to_string();
        if name=="none"||name.is_empty() { return Ok(None); }
        if registry.contains(&name) { Ok(Some(name)) } else { Ok(None) }
    }

    /// AI 推断（仅用预构建的能力定义列表，不持有 registry 引用）
    pub async fn infer_via_ai_with_defs(task: &str, defs: &[aion_types::capability_registry::CapabilityDefinition]) -> Option<String> {
        let caps: Vec<String> = defs.iter().map(|d| format!("- {} : {}", d.name, d.description)).collect();
        let sys = format!("You are a capability classifier. Return ONLY the best matching capability name from this list, or 'none'.\n\nCapabilities:\n{}", caps.join("\n"));
        let content = ai_chat(|model| json!({"model":model,"messages":[{"role":"system","content":sys},{"role":"user","content":task}],"temperature":0.0,"max_tokens":32})).await?;
        let name = content.lines().next().unwrap_or("").trim().trim_matches(|c: char| !c.is_alphanumeric()&&c!='_').to_string();
        if name=="none"||name.is_empty() { return None; }
        if defs.iter().any(|d| d.name == name) { Some(name) } else { None }
    }

    /// 同步版的 AI 发现（为 route_with_context 分阶段调用准备）
    /// 注意：此函数使用 blocking HTTP 调用（仅在 tokio runtime 外或 spawn_blocking 中安全调用）
    /// MVP 阶段：返回 None 跳过发现（避免在 async 中 blocking）
    pub fn ai_discover_sync(_task: &str, _registry: &mut CapabilityRegistry, _cap_dir: &std::path::Path) -> Option<String> {
        // MVP: 跳过同步 AI 发现，交由关键词 + async AI 两步覆盖
        // Phase 2 将使用 tokio::sync::Mutex 后可直接 async 调用
        None
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
            (&["rag","knowledge","知识库","document ingest"],"rag_ingest"),
            (&["rag query","knowledge query","知识检索","ask knowledge"],"rag_query"),
            (&["rag status","knowledge status","知识库状态"],"rag_status"),
            (&["mcp","mcp call","mcp tool"],"mcp_call"),
            (&["路由","route_task","任务路由","选引擎","route task","选择引擎"],"route_task"),
            // 多模型编排
            (&["parallel","并行求解","多引擎"],"ai_parallel_solve"),
            (&["vote","投票","三方投票"],"ai_triple_vote"),
            (&["collaborate","协作","共识","讨论"],"ai_smart_collaborate"),
            (&["research","研究","调研","综述"],"ai_research"),
            (&["code generate","生成代码","写代码"],"ai_code_generate"),
            (&["triangle review","三角审查"],"ai_triangle_review"),
            (&["cross review","交叉审查"],"ai_cross_review"),
            (&["serial optimize","串行优化"],"ai_serial_optimize"),
            (&["long context","超长文本","长上下文"],"ai_long_context"),
            // Agent
            (&["delegate","委派","分配任务"],"agent_delegate"),
            (&["broadcast","广播","通知全部"],"agent_broadcast"),
            (&["gather","收集","汇总回复"],"agent_gather"),
            (&["agent status","agent 状态"],"agent_status"),
            // 记忆
            (&["remember","记住","保存记忆"],"memory_remember"),
            (&["recall","回忆","查记忆"],"memory_recall"),
            (&["distill","记忆整理","压缩记忆"],"memory_distill"),
            (&["team share","团队共享","共享记忆"],"memory_team_share"),
            // Pipeline
            (&["pipeline","流水线","串行执行"],"task_pipeline"),
            (&["race","竞赛","最快返回"],"task_race"),
            // 其他
            (&["json query","jsonpath","json 查询"],"json_query"),
            (&["regex","正则","pattern match"],"regex_match"),
            (&["skill report","技能报告","使用统计"],"skill_report"),
            (&["spec driven","规格驱动","分阶段","迁移计划"],"spec_driven"),
            (&["discovery","级联搜索","多源搜索"],"discovery_search"),
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
