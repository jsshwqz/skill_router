//! 执行器插件系统
//!
//! 所有 builtin 技能通过 `BuiltinSkill` trait 注册到 `BuiltinRegistry`，
//! 替代原有 executor.rs 中的巨型 match 分支。

pub mod parsing;
pub mod text;
pub mod web;
pub mod memory;
pub mod ai;
pub mod agent;
pub mod pipeline;
pub mod new_skills;
pub mod mcp;
pub mod rag;
pub mod orchestrator;
pub mod spec_driven;
pub mod task_router;

use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

use aion_types::types::{ExecutionContext, SkillDefinition};

/// 所有 builtin 技能必须实现此 trait
#[async_trait::async_trait]
pub trait BuiltinSkill: Send + Sync {
    /// builtin 名称（对应 entrypoint 中 "builtin:" 后的部分）
    fn name(&self) -> &'static str;

    /// 执行技能
    async fn execute(
        &self,
        skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value>;
}

/// Builtin 注册表
pub struct BuiltinRegistry {
    skills: HashMap<&'static str, Box<dyn BuiltinSkill>>,
}

impl BuiltinRegistry {
    /// 创建空注册表
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 注册一个 builtin 技能
    pub fn register(&mut self, skill: Box<dyn BuiltinSkill>) {
        self.skills.insert(skill.name(), skill);
    }

    /// 查找 builtin 技能
    pub fn get(&self, name: &str) -> Option<&dyn BuiltinSkill> {
        self.skills.get(name).map(|b| b.as_ref())
    }

    /// 创建包含所有内置技能的默认注册表
    pub fn default_registry() -> Self {
        let mut reg = Self::new();

        // 解析类
        reg.register(Box::new(parsing::YamlParse));
        reg.register(Box::new(parsing::JsonParse));
        reg.register(Box::new(parsing::TomlParse));
        reg.register(Box::new(parsing::CsvParse));
        reg.register(Box::new(parsing::PdfParse));

        // 文本类
        reg.register(Box::new(text::TextDiff));
        reg.register(Box::new(text::TextEmbed));
        reg.register(Box::new(text::MarkdownRender));

        // 网络类
        reg.register(Box::new(web::WebSearch));
        reg.register(Box::new(web::HttpFetch));
        reg.register(Box::new(web::DiscoverySearch));

        // 记忆类
        reg.register(Box::new(memory::MemoryRemember));
        reg.register(Box::new(memory::MemoryRecall));
        reg.register(Box::new(memory::MemoryDistill));
        reg.register(Box::new(memory::MemoryTeamShare));

        // AI 类
        reg.register(Box::new(ai::AiTask));

        // Agent 类
        reg.register(Box::new(agent::AgentDelegate));
        reg.register(Box::new(agent::AgentBroadcast));
        reg.register(Box::new(agent::AgentGather));
        reg.register(Box::new(agent::AgentStatus));

        // 管道类
        reg.register(Box::new(pipeline::TaskPipeline));
        reg.register(Box::new(pipeline::TaskRace));

        // 新技能
        reg.register(Box::new(new_skills::Echo));
        reg.register(Box::new(new_skills::SpaceNavigation));
        reg.register(Box::new(new_skills::JsonQuery));
        reg.register(Box::new(new_skills::RegexMatch));
        reg.register(Box::new(new_skills::CodeLint));
        reg.register(Box::new(new_skills::CodeTest));
        reg.register(Box::new(new_skills::SkillReport));

        // MCP 调用
        reg.register(Box::new(mcp::McpCall));

        // RAG 检索增强
        reg.register(Box::new(rag::RagIngest));
        reg.register(Box::new(rag::RagQuery));
        reg.register(Box::new(rag::RagStatus));

        // 多模型编排（替代 Python ai-orchestrator）
        reg.register(Box::new(orchestrator::AsyncTaskQuery));
        reg.register(Box::new(orchestrator::AiParallelSolve));
        reg.register(Box::new(orchestrator::AiTripleVote));
        reg.register(Box::new(orchestrator::AiTriangleReview));
        reg.register(Box::new(orchestrator::AiCodeGenerate));
        reg.register(Box::new(orchestrator::AiSmartCollaborate));
        reg.register(Box::new(orchestrator::AiResearch));
        reg.register(Box::new(orchestrator::AiSerialOptimize));
        reg.register(Box::new(orchestrator::AiLongContext));
        reg.register(Box::new(orchestrator::AiCrossReview));

        // Spec-Driven 规格驱动开发
        reg.register(Box::new(spec_driven::SpecDriven));

        // AI 任务路由器
        reg.register(Box::new(task_router::RouteTaskBuiltin));

        reg
    }
}

// ── 公共工具函数 ────────────────────────────────────────────────────────────

/// 从 context 中提取 text/input 字段
pub(crate) fn require_text(ctx: &ExecutionContext) -> Result<String> {
    ctx.context["text"]
        .as_str()
        .or_else(|| ctx.context["input"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("context.text is required for this skill"))
}

/// 从 context 中提取 text/input 字段，回退到 task
pub(crate) fn extract_text(ctx: &ExecutionContext) -> String {
    ctx.context["text"]
        .as_str()
        .or_else(|| ctx.context["input"].as_str())
        .unwrap_or(&ctx.task)
        .to_string()
}

/// 简易 UUID（基于纳秒时间戳）
pub(crate) fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:016x}", nanos)
}

/// 当前 epoch 毫秒
pub(crate) fn now_epoch_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 简易 URL 编码
pub(crate) fn urlencoding_simple(s: &str) -> String {
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

/// YAML 标量解析
pub(crate) fn yaml_scalar(s: &str) -> Value {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    match s {
        "null" | "~" => Value::Null,
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                serde_json::json!(n)
            } else if let Ok(f) = s.parse::<f64>() {
                serde_json::json!(f)
            } else {
                Value::String(s.to_string())
            }
        }
    }
}
