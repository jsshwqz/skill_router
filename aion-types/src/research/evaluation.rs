//! 模型质量评估的数据结构和流水线。
//!
//! 提供困惑度（PPL）、KL 散度、连贯性评分、多样性指标等
//! 多维评估能力。

use serde::{Deserialize, Serialize};

/// 评估维度枚举。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MetricCategory {
    /// 语言建模基本指标
    #[default]
    LanguageModeling,
    /// 分布相似度
    DistributionSimilarity,
    /// 输出质量
    OutputQuality,
    /// 鲁棒性
    Robustness,
    /// 自定义维度
    Custom(String),
}

/// 单个评估指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// 指标名称
    pub name: String,
    /// 指标类别
    #[serde(default)]
    pub category: MetricCategory,
    /// 指标值
    pub value: f64,
    /// 单位（如 "tokens/s"、"bits"）
    #[serde(default)]
    pub unit: String,
    /// 是否越低越好
    #[serde(default = "default_lower_is_better")]
    pub lower_is_better: bool,
    /// 正常范围（可选）
    #[serde(default)]
    pub expected_range: Option<(f64, f64)>,
}

fn default_lower_is_better() -> bool {
    true
}

/// 评估样本输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSample {
    /// 文本内容
    pub text: String,
    /// 可选的参考输出（用于有监督评估）
    pub reference: Option<String>,
    /// 样本元数据
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// 评估任务定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTask {
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: String,
    /// 使用的指标集合
    pub metrics: Vec<MetricId>,
    /// 评估样本
    pub samples: Vec<EvalSample>,
    /// 采样策略
    #[serde(default)]
    pub sampling: Option<SamplingStrategy>,
}

/// 指标引用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricId {
    pub name: String,
    pub params: std::collections::HashMap<String, serde_json::Value>,
}

/// 采样策略。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SamplingStrategy {
    /// 均匀随机采样
    Random { seed: u64, max_samples: usize },
    /// 按 perplexity 排序后分层采样
    StratifiedPpl { num_strata: usize },
    /// 固定样本集
    Fixed,
}

/// 单次评估的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    /// 评估任务名称
    pub task_name: String,
    /// 评估时间戳
    pub timestamp: String,
    /// 模型标识
    pub model_id: String,
    /// 各指标的聚合结果
    pub metrics: Vec<MetricAggregation>,
    /// 每个样本的详细结果
    #[serde(default)]
    pub sample_results: Vec<SampleResult>,
    /// 评估配置
    pub config: EvalConfig,
}

/// 指标聚合结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAggregation {
    pub metric_name: String,
    pub mean: f64,
    pub std: f64,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub percentile_90: f64,
    pub percentile_99: f64,
    pub count: usize,
}

/// 单个样本的评估结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleResult {
    pub text_hash: String,
    pub metrics: std::collections::HashMap<String, f64>,
    pub status: EvalStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvalStatus {
    Success,
    Skipped,
    Error(String),
    Timeout,
}

/// 评估配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    pub batch_size: usize,
    pub max_tokens: usize,
    pub temperature: f64,
    pub top_p: f64,
    pub num_evaluations: usize,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.9,
            num_evaluations: 100,
        }
    }
}

/// 评估流水线状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalPipelineState {
    pub completed_tasks: Vec<String>,
    pub pending_tasks: Vec<String>,
    pub failed_tasks: Vec<(String, String)>, // (task_name, error)
    pub total_metrics_computed: usize,
}
