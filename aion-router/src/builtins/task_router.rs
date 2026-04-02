//! AI 任务路由器 builtin
//!
//! 根据任务描述，自动选择最合适的 AI 引擎，返回可直接执行的
//! aion-forge 工具调用参数。三层触发：结构快筛 → 关键词×weight → AI 兜底。

use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use tracing::{info, warn};

use aion_types::route_types::*;
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

// ─── 全局路由规则（进程生命周期加载一次） ───

static ROUTER_DATA: OnceLock<RouterFile> = OnceLock::new();

/// 从项目根目录或 exe 同级目录加载 router.json
fn load_router_file() -> RouterFile {
    let candidates = [
        std::env::current_dir()
            .unwrap_or_default()
            .join("router.json"),
        std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("router.json"),
    ];
    for path in &candidates {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<RouterFile>(&content) {
                    Ok(rf) => {
                        info!(
                            "route_task: loaded {} rules from {}",
                            rf.rules.len(),
                            path.display()
                        );
                        return rf;
                    }
                    Err(e) => warn!("route_task: parse error in {}: {}", path.display(), e),
                },
                Err(e) => warn!("route_task: read error {}: {}", path.display(), e),
            }
        }
    }
    warn!("route_task: router.json not found, using empty rules");
    RouterFile {
        config: RouterConfig::default(),
        rules: vec![],
    }
}

fn router_data() -> &'static RouterFile {
    ROUTER_DATA.get_or_init(load_router_file)
}

// ─── 第一层：结构特征快筛 ───

/// 代码文件扩展名正则
fn code_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(```|\.(?:rs|py|js|ts|go|java|cpp|c|rb|swift|kt)\b)").unwrap()
    })
}

/// URL 正则
fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"https?://").unwrap())
}

fn struct_scan(task: &str, hints: &Option<RouteHints>) -> StructFeatures {
    let has_code_from_hints = hints
        .as_ref()
        .and_then(|h| h.has_code)
        .unwrap_or(false);
    let has_code = has_code_from_hints || code_regex().is_match(task);

    let doc_pages = hints.as_ref().and_then(|h| h.doc_size_pages).unwrap_or(0);
    let giant_doc = doc_pages > 80;

    let search_likely = url_regex().is_match(task);

    StructFeatures {
        has_code,
        giant_doc,
        search_likely,
    }
}

// ─── 第二层：关键词 × weight 匹配 ───

struct MatchCandidate {
    rule_idx: usize,
    weight: u32,
    matched_keywords: Vec<String>,
}

fn keyword_weight_match(
    task: &str,
    features: &StructFeatures,
    rules: &[RouteRule],
) -> Vec<MatchCandidate> {
    let task_lower = task.to_lowercase();
    let mut candidates: Vec<MatchCandidate> = Vec::new();

    for (idx, rule) in rules.iter().enumerate() {
        let matched: Vec<String> = rule
            .keywords
            .iter()
            .filter(|kw: &&String| kw.chars().count() >= 2) // 忽略单字关键词，防止误匹配
            .filter(|kw: &&String| task_lower.contains(&kw.to_lowercase()))
            .cloned()
            .collect();

        if matched.is_empty() {
            continue;
        }

        // 基础 weight + 结构特征加权
        let mut effective_weight = rule.weight;
        if features.has_code && rule.category == Category::Code {
            effective_weight = effective_weight.saturating_add(5);
        }
        if features.giant_doc && rule.id.contains("giant") {
            effective_weight = effective_weight.saturating_add(10);
        }
        if features.search_likely && rule.category == Category::Search {
            effective_weight = effective_weight.saturating_add(5);
        }

        candidates.push(MatchCandidate {
            rule_idx: idx,
            weight: effective_weight,
            matched_keywords: matched,
        });
    }

    // 按 weight 降序排列
    candidates.sort_by(|a, b| b.weight.cmp(&a.weight));
    candidates
}

// ─── 模板渲染 ───

/// 根据 aion_tool 签名确定 task 注入的字段名
fn task_field_for_tool(tool: &str) -> &'static str {
    match tool {
        "ai_code_generate" => "task",
        "ai_parallel_solve" => "problem",
        "ai_serial_optimize" => "code",
        "ai_cross_review" => "code",
        "ai_triangle_review" => "code",
        "ai_triple_vote" => "problem",
        "ai_smart_collaborate" => "task",
        "ai_research" => "topic",
        _ => "task",
    }
}

fn render_params(template: &Value, tool: &str, task: &str) -> Value {
    let mut params = template.clone();
    if let Some(obj) = params.as_object_mut() {
        let field = task_field_for_tool(tool);
        obj.insert(field.to_string(), Value::String(task.to_string()));
    } else {
        // template 为 null 或非 object，构建最小参数
        let field = task_field_for_tool(tool);
        params = json!({ field: task });
    }
    params
}

// ─── access_ok 判定 ───

fn check_access(rule: &RouteRule, config: &RouterConfig) -> bool {
    if !config.access_restricted {
        return true;
    }
    if !rule.requires_external {
        return true;
    }
    // 检查受限服务
    match rule.engine.as_str() {
        "google" => !config.restricted_services.google_external,
        "openai" if rule.id.contains("realtime") || rule.id.contains("voice") => {
            !config.restricted_services.openai_realtime
        }
        _ => true,
    }
}

// ─── 构建 RouteDecision ───

fn build_decision(
    rule: &RouteRule,
    task: &str,
    config: &RouterConfig,
    conflict_note: Option<String>,
) -> RouteDecision {
    let aion_params = match (&rule.aion_tool, &rule.aion_params_template) {
        (Some(tool), Some(template)) => Some(render_params(template, tool, task)),
        (Some(tool), None) => {
            let field = task_field_for_tool(tool);
            Some(json!({ field: task }))
        }
        _ => None,
    };

    RouteDecision {
        rule_id: rule.id.clone(),
        engine: rule.engine.clone(),
        model: rule.model.clone(),
        requires_external: rule.requires_external,
        aion_tool: rule.aion_tool.clone(),
        aion_params,
        external_hint: rule.external_hint.clone(),
        fallback_chain: rule.fallback_chain.clone(),
        access_ok: check_access(rule, config),
        conflict_note,
    }
}

// ─── 默认 fallback ───

fn default_fallback(task: &str) -> RouteDecision {
    RouteDecision {
        rule_id: "default-fallback".to_string(),
        engine: "anthropic".to_string(),
        model: "claude-sonnet-4-5".to_string(),
        requires_external: false,
        aion_tool: Some("ai_parallel_solve".to_string()),
        aion_params: Some(json!({
            "problem": task,
            "engines": ["claude"]
        })),
        external_hint: None,
        fallback_chain: vec!["openai".to_string(), "gemini".to_string()],
        access_ok: true,
        conflict_note: Some("未匹配任何规则，使用默认 fallback".to_string()),
    }
}

// ─── 第三层：AI 意图兜底（passthrough 模式） ───

/// 构建 passthrough 指令，让宿主 LLM 从规则列表中选择
fn build_ai_fallback_instruction(task: &str, rules: &[RouteRule]) -> Value {
    let rule_list: Vec<Value> = rules
        .iter()
        .map(|r| {
            json!({
                "rule_id": r.id,
                "category": r.category.to_string(),
                "note": r.note.as_deref().unwrap_or(""),
                "keywords_hint": r.keywords.join(", ")
            })
        })
        .collect();

    json!({
        "type": "passthrough",
        "instruction": format!(
            "关键词匹配未命中。请根据任务描述，从以下规则中选择最匹配的 rule_id。\n\
             如果都不匹配，返回 \"none\"。只返回 rule_id 字符串。\n\n\
             用户任务：{}", task
        ),
        "available_rules": rule_list,
        "expected_response": "rule_id string or 'none'"
    })
}

// ─── 路由结果（支持直接决策 + passthrough 两种模式） ───

enum RouteOutcome {
    /// 第一/二层命中，直接返回决策
    Decision(RouteDecision),
    /// 第三层未命中关键词，返回 passthrough 指令让宿主 LLM 决策
    Passthrough(Value),
}

// ─── 核心路由函数 ───

fn route_task_core(task: &str, hints: Option<RouteHints>) -> RouteOutcome {
    let data = router_data();
    let config = &data.config;
    let rules = &data.rules;

    if rules.is_empty() {
        return RouteOutcome::Decision(default_fallback(task));
    }

    // 第一层：结构特征快筛
    let features = struct_scan(task, &hints);

    // 第二层：关键词 × weight 匹配
    let candidates = keyword_weight_match(task, &features, rules);

    if !candidates.is_empty() {
        let best = &candidates[0];
        let rule = &rules[best.rule_idx];

        // 构建 conflict_note
        let conflict_note = if candidates.len() > 1 {
            let runner_up = &candidates[1];
            let runner_rule = &rules[runner_up.rule_idx];
            Some(format!(
                "命中 {} 条规则，选择 '{}' (weight={})，次选 '{}' (weight={})",
                candidates.len(),
                rule.id,
                best.weight,
                runner_rule.id,
                runner_up.weight
            ))
        } else {
            None
        };

        info!(
            "route_task: matched '{}' (weight={}, keywords={:?})",
            rule.id, best.weight, best.matched_keywords
        );

        return RouteOutcome::Decision(build_decision(rule, task, config, conflict_note));
    }

    // 第三层：passthrough 模式，让宿主 LLM 选择
    info!("route_task: no keyword match, returning passthrough for host LLM");
    RouteOutcome::Passthrough(build_ai_fallback_instruction(task, rules))
}

/// 宿主 LLM 回传 rule_id 后，解析并构建最终决策
fn resolve_passthrough(rule_id: &str, task: &str) -> RouteDecision {
    let data = router_data();
    let config = &data.config;
    let rules = &data.rules;

    // 精确匹配
    if let Some(rule) = rules.iter().find(|r| r.id == rule_id) {
        info!("route_task: passthrough resolved to '{}'", rule.id);
        return build_decision(
            rule,
            task,
            config,
            Some(format!("关键词未命中，由宿主 LLM 选择 '{}'", rule.id)),
        );
    }

    // 包含匹配（取最长 id）
    let rule_id_lower = rule_id.to_lowercase();
    let mut best: Option<&RouteRule> = None;
    for r in rules {
        if rule_id_lower.contains(&r.id) {
            match best {
                Some(b) if b.id.len() >= r.id.len() => {}
                _ => best = Some(r),
            }
        }
    }

    if let Some(rule) = best {
        info!("route_task: passthrough fuzzy resolved to '{}'", rule.id);
        return build_decision(
            rule,
            task,
            config,
            Some(format!("关键词未命中，由宿主 LLM 选择 '{}'", rule.id)),
        );
    }

    // 宿主也选不出来，走默认
    info!("route_task: passthrough returned unknown rule_id '{}', using default", rule_id);
    default_fallback(task)
}

// ─── Builtin 实现 ───

pub struct RouteTaskBuiltin;

#[async_trait::async_trait]
impl BuiltinSkill for RouteTaskBuiltin {
    fn name(&self) -> &'static str {
        "route_task"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        // 提取 task 参数
        let task = context.context["task"]
            .as_str()
            .or_else(|| context.context["text"].as_str())
            .unwrap_or(&context.task);

        if task.trim().is_empty() {
            return Err(anyhow!("route_task requires non-empty 'task' parameter"));
        }

        // 检查是否为 passthrough 回传（宿主 LLM 选择了 rule_id 后回传）
        if let Some(rule_id) = context.context.get("resolve_rule_id").and_then(|v| v.as_str()) {
            let decision = resolve_passthrough(rule_id, task);
            let result = serde_json::to_value(&decision)
                .map_err(|e| anyhow!("failed to serialize RouteDecision: {}", e))?;
            return Ok(json!({
                "capability": "route_task",
                "output": result,
                "task": task,
                "provider": "builtin"
            }));
        }

        // 提取 hints 参数
        let hints: Option<RouteHints> = context
            .context
            .get("hints")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // 执行路由
        match route_task_core(task, hints) {
            RouteOutcome::Decision(decision) => {
                let result = serde_json::to_value(&decision)
                    .map_err(|e| anyhow!("failed to serialize RouteDecision: {}", e))?;
                Ok(json!({
                    "capability": "route_task",
                    "output": result,
                    "task": task,
                    "provider": "builtin"
                }))
            }
            RouteOutcome::Passthrough(instruction) => {
                // 返回 passthrough 指令，宿主 LLM 选择 rule_id 后
                // 再次调用 route_task 并传入 resolve_rule_id 参数
                Ok(json!({
                    "capability": "route_task",
                    "output": instruction,
                    "task": task,
                    "provider": "builtin",
                    "needs_resolve": true
                }))
            }
        }
    }
}
