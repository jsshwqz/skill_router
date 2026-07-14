//! 迭代精炼闭环的数据结构。
//!
//! 提供探测→分析→修正→验证的循环优化框架。

use crate::research::evaluation::{EvalConfig, EvalResult};
use serde::{Deserialize, Serialize};

/// 精炼阶段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefineStage {
    /// 探测：收集当前状态的激活/响应
    Probe,
    /// 分析：识别需要修正的方向
    Analyze,
    /// 修正：应用修改操作
    Modify,
    /// 验证：评估修正后的质量
    Verify,
    /// 完成：精炼结束
    Complete,
    /// 失败：无法继续
    Failed(String),
}

/// 精炼操作类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModifyOp {
    /// 沿指定方向施加缩放因子
    DirectionalScale { direction_index: usize, scale_factor: f64 },
    /// 移除指定方向
    DirectionRemoval { direction_index: usize },
    /// 投影到子空间正交补
    SubspaceProjection {
        subspace_indices: Vec<usize>,
        projection_strength: f64,
    },
    /// 自定义操作
    Custom {
        op_name: String,
        params: std::collections::HashMap<String, serde_json::Value>,
    },
}

/// 单次精炼迭代的记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineIteration {
    /// 迭代编号
    pub iteration: usize,
    /// 当前阶段
    pub stage: RefineStage,
    /// 探测阶段的激活统计
    pub probe_stats: Option<ProbeStats>,
    /// 分析阶段的发现
    pub analysis: Option<AnalysisResult>,
    /// 应用的修正操作
    pub modifications: Vec<ModifyRecord>,
    /// 验证阶段的评估结果
    pub verification: Option<EvalResult>,
    /// 是否达到收敛标准
    pub converged: bool,
    /// 错误信息（如果有）
    pub error: Option<String>,
}

/// 探测阶段的统计信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStats {
    /// 激活均值
    pub activation_mean: f64,
    /// 激活方差
    pub activation_var: f64,
    /// 最大激活值
    pub max_activation: f64,
    /// 稀疏度（零值比例）
    pub sparsity: f64,
}

/// 分析结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// 识别出的关键方向索引
    pub key_directions: Vec<usize>,
    /// 每个方向的能量占比
    pub energy_ratios: Vec<f64>,
    /// 分析置信度
    pub confidence: f64,
    /// 建议的操作
    pub suggested_ops: Vec<ModifyOp>,
}

/// 修正操作记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyRecord {
    pub op: ModifyOp,
    pub target_layer: String,
    pub applied_at_iteration: usize,
    pub magnitude: f64,
}

/// 精炼配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineConfig {
    /// 最大迭代次数
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// 收敛阈值（连续 N 次迭代改善小于此值则停止）
    #[serde(default = "default_convergence_window")]
    pub convergence_window: usize,
    /// 收敛容忍度
    #[serde(default = "default_convergence_tol")]
    pub convergence_tolerance: f64,
    /// 评估配置
    #[serde(default)]
    pub eval_config: EvalConfig,
    /// 要精炼的目标层（空 = 全部）
    #[serde(default)]
    pub target_layers: Vec<String>,
    /// 修正操作的强度上限
    #[serde(default = "default_max_magnitude")]
    pub max_magnitude: f64,
}

fn default_max_iterations() -> usize {
    5
}
fn default_convergence_window() -> usize {
    3
}
fn default_convergence_tol() -> f64 {
    1e-4
}
fn default_max_magnitude() -> f64 {
    1.0
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            convergence_window: default_convergence_window(),
            convergence_tolerance: default_convergence_tol(),
            eval_config: EvalConfig::default(),
            target_layers: vec![],
            max_magnitude: default_max_magnitude(),
        }
    }
}

/// 精炼会话的完整记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineSession {
    pub session_id: String,
    pub model_id: String,
    pub config: RefineConfig,
    pub iterations: Vec<RefineIteration>,
    pub final_state: RefineFinalState,
}

/// 精炼完成后的最终状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineFinalState {
    pub total_iterations: usize,
    pub converged: bool,
    pub convergence_reason: ConvergenceReason,
    pub quality_improvement: QualityDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDelta {
    pub baseline_score: f64,
    pub final_score: f64,
    pub improvement: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConvergenceReason {
    MaxIterationsReached,
    ToleranceMet,
    NoFurtherImprovement,
    StoppedByUser,
    Error(String),
}
