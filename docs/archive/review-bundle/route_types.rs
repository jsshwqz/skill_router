//! AI 任务路由器数据结构
//!
//! 定义 route_task 工具的输入、输出和规则格式。
//! 路由规则从外部 router.json 加载，运行时匹配。

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── 分类枚举 ───

/// 路由规则分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Category {
    Code,
    Creative,
    Analysis,
    Search,
    Voice,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "CODE"),
            Self::Creative => write!(f, "CREATIVE"),
            Self::Analysis => write!(f, "ANALYSIS"),
            Self::Search => write!(f, "SEARCH"),
            Self::Voice => write!(f, "VOICE"),
        }
    }
}

// ─── 外部工具提示 ───

/// 当 aion-forge 没有对应工具时，提供外部调用建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalHint {
    pub api: String,
    pub description: String,
    #[serde(default)]
    pub access_note: Option<String>,
}

// ─── 路由规则 ───

/// router.json 中的单条路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    pub id: String,
    pub category: Category,
    pub keywords: Vec<String>,
    pub weight: u32,
    pub engine: String,
    pub model: String,
    #[serde(default)]
    pub aion_tool: Option<String>,
    #[serde(default)]
    pub aion_params_template: Option<Value>,
    #[serde(default)]
    pub requires_external: bool,
    #[serde(default)]
    pub external_hint: Option<ExternalHint>,
    #[serde(default)]
    pub fallback_chain: Vec<String>,
    #[serde(default)]
    pub model_verified_date: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

// ─── 路由决策（输出） ───

/// route_task 的返回结果，Orchestrator 拿到后可直接执行
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    pub rule_id: String,
    pub engine: String,
    pub model: String,
    pub requires_external: bool,
    #[serde(default)]
    pub aion_tool: Option<String>,
    #[serde(default)]
    pub aion_params: Option<Value>,
    #[serde(default)]
    pub external_hint: Option<ExternalHint>,
    pub fallback_chain: Vec<String>,
    pub access_ok: bool,
    #[serde(default)]
    pub conflict_note: Option<String>,
}

// ─── 结构特征（第一层快筛产出） ───

/// 结构特征快筛结果
#[derive(Debug, Clone, Default)]
pub struct StructFeatures {
    pub has_code: bool,
    pub giant_doc: bool,
    pub search_likely: bool,
}

// ─── 全局配置 ───

/// 受限服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictedServices {
    #[serde(default)]
    pub google_external: bool,
    #[serde(default)]
    pub openai_realtime: bool,
}

impl Default for RestrictedServices {
    fn default() -> Self {
        Self {
            google_external: true,
            openai_realtime: false,
        }
    }
}

/// 路由器全局配置（从 router.json 的 config 段读取）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    #[serde(default)]
    pub access_restricted: bool,
    #[serde(default)]
    pub restricted_services: RestrictedServices,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            access_restricted: true,
            restricted_services: RestrictedServices::default(),
        }
    }
}

// ─── router.json 顶层结构 ───

/// router.json 文件的完整结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterFile {
    #[serde(default)]
    pub config: RouterConfig,
    pub rules: Vec<RouteRule>,
}

// ─── 输入结构 ───

/// route_task 工具的 hints 参数
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RouteHints {
    #[serde(default)]
    pub doc_size_pages: Option<u32>,
    #[serde(default)]
    pub has_code: Option<bool>,
}

/// route_task 工具的完整输入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTaskInput {
    pub task: String,
    #[serde(default)]
    pub hints: Option<RouteHints>,
}
