//! 模型谱分析的数据结构。
//!
//! 提供权重矩阵的奇异值分解（SVD）、特征值分析、主成分分析（PCA）
//! 等相关的数据类型，用于理解模型内部的表示空间和方向特性。

use serde::{Deserialize, Serialize};

/// 矩阵的奇异值分解结果。
///
/// 存储 U、Σ、V^T 三个因子矩阵以及奇异值本身。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvdfResult {
    /// 左奇异向量矩阵 (m × k)，每列是一个主成分方向
    pub u: Vec<Vec<f64>>,
    /// 奇异值数组（降序排列）
    pub singular_values: Vec<f64>,
    /// 右奇异向量矩阵 (n × k)，每列是一个输入方向
    pub vt: Vec<Vec<f64>>,
    /// 保留的成分数量
    pub rank: usize,
}

/// 特征值分解结果（对称矩阵专用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EigenResult {
    /// 特征值数组（降序排列）
    pub eigenvalues: Vec<f64>,
    /// 对应的特征向量（每列一个）
    pub eigenvectors: Vec<Vec<f64>>,
}

/// 主成分分析结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcaResult {
    /// 各主成分的方差解释比例（累计）
    pub explained_variance_ratio: Vec<f64>,
    /// 主成分得分矩阵 (n_samples × n_components)
    pub components: Vec<Vec<f64>>,
    /// 均值向量
    pub mean: Vec<f64>,
}

/// 谱密度估计结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralDensity {
    /// 谱 bin 中心
    pub bins: Vec<f64>,
    /// 对应 bin 的密度估计值
    pub density: Vec<f64>,
    /// 使用的带宽（高斯核）
    pub bandwidth: f64,
}

/// 权重矩阵的统计摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixStats {
    /// 矩阵行数
    pub rows: usize,
    /// 矩阵列数
    pub cols: usize,
    /// Frobenius 范数
    pub frobenius_norm: f64,
    /// 谱范数（最大奇异值）
    pub spectral_norm: f64,
    /// 条件数（最大/最小奇异值比）
    pub condition_number: f64,
    /// 秩（基于阈值的有效秩）
    pub effective_rank: f64,
    /// 熵（奇异值分布的香农熵）
    pub entropy: f64,
}

/// 谱分析的配置参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralAnalysisConfig {
    /// 计算的最大奇异值数量（0 = 全部）
    #[serde(default = "default_max_sv")]
    pub max_singular_values: usize,
    /// 有效秩判定阈值
    #[serde(default = "default_rank_threshold")]
    pub rank_threshold: f64,
    /// 是否计算谱密度估计
    #[serde(default)]
    pub compute_density: bool,
    /// 是否进行主成分分析
    #[serde(default)]
    pub compute_pca: bool,
}

impl Default for SpectralAnalysisConfig {
    fn default() -> Self {
        Self {
            max_singular_values: default_max_sv(),
            rank_threshold: default_rank_threshold(),
            compute_density: false,
            compute_pca: false,
        }
    }
}

fn default_max_sv() -> usize {
    100
}
fn default_rank_threshold() -> f64 {
    1e-6
}

/// 层级别的谱分析结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSpectralResult {
    /// 层名称（如 "model.layers.0.self_attn.q_proj"）
    pub layer_name: String,
    /// 矩阵形状
    pub shape: (usize, usize),
    /// 矩阵统计摘要
    pub stats: MatrixStats,
    /// SVD 结果（如果请求了）
    pub svd: Option<SvdfResult>,
    /// 特征值结果（如果是对称矩阵且请求了）
    pub eigen: Option<EigenResult>,
    /// PCA 结果
    pub pca: Option<PcaResult>,
    /// 谱密度
    pub spectral_density: Option<SpectralDensity>,
}

/// 跨层谱分析汇总。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossLayerSpectralSummary {
    /// 各层的最大奇异值
    pub max_singular_values: Vec<(String, f64)>,
    /// 各层的有效秩
    pub effective_ranks: Vec<(String, f64)>,
    /// 各层的谱熵
    pub spectral_entropies: Vec<(String, f64)>,
    /// 全局统计
    pub global: GlobalSpectralStats,
}

/// 全局谱统计量。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSpectralStats {
    /// 所有层最大奇异值的均值和标准差
    pub max_sv_mean: f64,
    pub max_sv_std: f64,
    /// 所有层有效秩的均值和标准差
    pub effective_rank_mean: f64,
    pub effective_rank_std: f64,
    /// 是否存在异常层（最大奇异值偏离均值 > 3σ）
    pub anomalous_layers: Vec<String>,
}
