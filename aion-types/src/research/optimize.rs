//! 超参优化的数据结构。
//!
//! 提供贝叶斯优化、网格搜索、随机搜索等策略的通用框架。

use serde::{Deserialize, Serialize};

/// 优化目标。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    /// 最大化指标
    Maximize(String),
    /// 最小化指标
    Minimize(String),
    /// 多目标（Pareto 前沿）
    MultiObjective(Vec<String>),
}

/// 参数搜索空间类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamSpace {
    /// 连续均匀分布 [low, high]
    Uniform { low: f64, high: f64 },
    /// 连续对数均匀分布 [low, high]（log-scale）
    LogUniform { low: f64, high: f64 },
    /// 离散整数范围 [low, high]
    IntUniform { low: i64, high: i64 },
    /// 离散对数整数范围
    IntLogUniform { low: i64, high: i64 },
    /// 分类变量
    Categorical { values: Vec<String> },
    /// 固定值
    Constant { value: serde_json::Value },
}

/// 优化配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// 优化算法
    #[serde(default = "default_algorithm")]
    pub algorithm: OptimizationAlgorithm,
    /// 最大评估次数
    #[serde(default = "default_max_trials")]
    pub max_trials: usize,
    /// 早期停止：连续 N 次无改善则停止
    #[serde(default = "default_early_stop_patience")]
    pub early_stop_patience: usize,
    /// 并行评估数量
    #[serde(default = "default_parallelism")]
    pub parallelism: usize,
    /// 随机种子
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// 参数搜索空间
    pub param_spaces: std::collections::HashMap<String, ParamSpace>,
    /// 优化目标
    pub objectives: Vec<Objective>,
    /// 约束条件（可选）
    #[serde(default)]
    pub constraints: Vec<OptimizationConstraint>,
}

fn default_algorithm() -> OptimizationAlgorithm {
    OptimizationAlgorithm::Tpe
}
fn default_max_trials() -> usize {
    50
}
fn default_early_stop_patience() -> usize {
    10
}
fn default_parallelism() -> usize {
    1
}
fn default_seed() -> u64 {
    42
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            algorithm: default_algorithm(),
            max_trials: default_max_trials(),
            early_stop_patience: default_early_stop_patience(),
            parallelism: default_parallelism(),
            seed: default_seed(),
            param_spaces: std::collections::HashMap::new(),
            objectives: vec![Objective::Minimize("score".to_string())],
            constraints: vec![],
        }
    }
}

/// 支持的优化算法。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationAlgorithm {
    /// 随机搜索
    Random,
    /// 网格搜索
    Grid,
    /// TPE (Tree-structured Parzen Estimator)
    Tpe,
    /// CMA-ES (Covariance Matrix Adaptation)
    CmaEs,
    /// Hyperband (早停策略)
    Hyperband,
}

/// 优化约束。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConstraint {
    pub expression: String,
    pub threshold: f64,
    pub kind: ConstraintKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintKind {
    LessThan,
    GreaterThan,
    EqualTo,
}

/// 单次试验的参数和结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    /// 试验编号
    pub trial: usize,
    /// 使用的参数
    pub params: std::collections::HashMap<String, serde_json::Value>,
    /// 目标函数值
    pub value: f64,
    /// 评估耗时（秒）
    pub duration_secs: f64,
    /// 试验状态
    pub state: TrialState,
    /// 中间结果（用于 Hyperband 等）
    #[serde(default)]
    pub intermediates: Vec<TrialIntermediate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrialState {
    Running,
    Complete,
    Failed(String),
    Pruned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialIntermediate {
    pub step: usize,
    pub value: f64,
}

/// 优化会话记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSession {
    pub session_id: String,
    pub config: OptimizationConfig,
    pub trials: Vec<TrialResult>,
    pub best_trial: Option<usize>,
    pub best_value: Option<f64>,
}

impl OptimizationSession {
    pub fn new(config: OptimizationConfig) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            config,
            trials: vec![],
            best_trial: None,
            best_value: None,
        }
    }

    /// 添加一次试验结果并更新最佳记录。
    ///
    /// 只有完成且结果为有限数的单目标试验才参与最佳值比较。
    /// 多目标试验需要完整的目标向量，不能由单个 `value` 正确排序，
    /// 因此本会话只保留其结果，不伪造“最佳试验”。
    pub fn add_trial(&mut self, trial: TrialResult) {
        let idx = self.trials.len();
        let eligible = trial.state == TrialState::Complete && trial.value.is_finite();
        self.trials.push(trial);

        if !eligible
            || self.config.objectives.len() != 1
            || matches!(self.config.objectives[0], Objective::MultiObjective(_))
        {
            return;
        }

        let candidate = self.trials[idx].value;
        let is_better = self.best_value.is_none_or(|best| match &self.config.objectives[0] {
            Objective::Minimize(_) => candidate < best,
            Objective::Maximize(_) => candidate > best,
            Objective::MultiObjective(_) => false,
        });
        if is_better {
            self.best_value = Some(candidate);
            self.best_trial = Some(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(trial: usize, value: f64, state: TrialState) -> TrialResult {
        TrialResult {
            trial,
            params: std::collections::HashMap::new(),
            value,
            duration_secs: 0.0,
            state,
            intermediates: Vec::new(),
        }
    }

    #[test]
    fn selects_best_trial_in_configured_direction() {
        for (objective, values, expected) in [
            (Objective::Minimize("loss".to_string()), [3.0, 1.0], 1),
            (Objective::Maximize("score".to_string()), [1.0, 3.0], 1),
        ] {
            let config = OptimizationConfig {
                objectives: vec![objective],
                ..Default::default()
            };
            let mut session = OptimizationSession::new(config);
            session.add_trial(trial(10, values[0], TrialState::Complete));
            session.add_trial(trial(20, values[1], TrialState::Complete));
            assert_eq!(session.best_trial, Some(expected));
        }
    }

    #[test]
    fn ignores_failed_pruned_running_and_non_finite_trials() {
        let mut session = OptimizationSession::new(OptimizationConfig::default());
        session.add_trial(trial(0, 1.0, TrialState::Complete));
        session.add_trial(trial(1, -10.0, TrialState::Failed("boom".to_string())));
        session.add_trial(trial(2, -20.0, TrialState::Pruned));
        session.add_trial(trial(3, -30.0, TrialState::Running));
        session.add_trial(trial(4, f64::NAN, TrialState::Complete));
        assert_eq!(session.best_trial, Some(0));
        assert_eq!(session.best_value, Some(1.0));
    }

    #[test]
    fn does_not_claim_a_best_trial_for_multi_objective_results() {
        let config = OptimizationConfig {
            objectives: vec![Objective::MultiObjective(vec![
                "quality".to_string(),
                "latency".to_string(),
            ])],
            ..Default::default()
        };
        let mut session = OptimizationSession::new(config);
        session.add_trial(trial(0, 1.0, TrialState::Complete));
        assert_eq!(session.best_trial, None);
        assert_eq!(session.best_value, None);
    }
}
