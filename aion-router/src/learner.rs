//! 技能学习引擎
//!
//! 持久化记录每次技能执行的成功率、延迟、使用频次，
//! 用于路由优化和技能自动进化。
//!
//! 数据存储在 `{workspace}/learning/skill_stats.json`。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::info;

/// 熔断器冷却期（秒）：Open 状态经过此时间后进入 HalfOpen
const CIRCUIT_COOLDOWN_SECS: u64 = 300; // 5 分钟

/// 熔断器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// 正常运行，允许所有请求
    Closed,
    /// 熔断打开，拒绝所有请求（等待冷却期）
    Open,
    /// 半开状态，允许少量试探请求（1 次）
    HalfOpen,
}

impl Default for CircuitState {
    fn default() -> Self {
        CircuitState::Closed
    }
}

/// 获取当前时间戳（epoch secs）
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 单个技能的累积统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStats {
    /// 总调用次数
    pub total: u64,
    /// 成功次数
    pub ok: u64,
    /// 失败次数
    pub fail: u64,
    /// 最近一次使用的时间戳（epoch secs）
    #[serde(default)]
    pub last_used: u64,
    /// 平均延迟（毫秒）
    #[serde(default)]
    pub avg_latency_ms: f64,
    /// 最近 N 次执行的延迟（用于趋势分析）
    #[serde(default)]
    pub recent_latencies: Vec<u64>,
    /// 连续失败次数（用于熔断）
    #[serde(default)]
    pub consecutive_failures: u32,
    /// 用户显式好评次数
    #[serde(default)]
    pub thumbs_up: u32,
    /// 用户显式差评次数
    #[serde(default)]
    pub thumbs_down: u32,
    /// 熔断器当前状态
    #[serde(default)]
    pub circuit_state: CircuitState,
    /// 熔断器进入 Open 状态的时间戳（epoch secs）
    #[serde(default)]
    pub circuit_opened_at: u64,
}

impl Default for SkillStats {
    fn default() -> Self {
        Self {
            total: 0,
            ok: 0,
            fail: 0,
            last_used: 0,
            avg_latency_ms: 0.0,
            recent_latencies: Vec::new(),
            consecutive_failures: 0,
            thumbs_up: 0,
            thumbs_down: 0,
            circuit_state: CircuitState::Closed,
            circuit_opened_at: 0,
        }
    }
}

impl SkillStats {
    /// 成功率（0.0 ~ 1.0）
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.5 // 未知技能给中等评分
        } else {
            self.ok as f64 / self.total as f64
        }
    }

    /// 综合质量评分（0.0 ~ 1.0），综合成功率 + 延迟 + 用户反馈
    pub fn quality_score(&self) -> f64 {
        let sr = self.success_rate();

        // 延迟惩罚：超过 5 秒的每秒扣 0.02
        let latency_penalty = if self.avg_latency_ms > 5000.0 {
            ((self.avg_latency_ms - 5000.0) / 1000.0 * 0.02).min(0.2)
        } else {
            0.0
        };

        // 用户反馈加成
        let feedback_total = self.thumbs_up + self.thumbs_down;
        let feedback_bonus = if feedback_total > 0 {
            (self.thumbs_up as f64 / feedback_total as f64 - 0.5) * 0.1
        } else {
            0.0
        };

        // 熔断惩罚
        let circuit_penalty = match self.effective_circuit_state() {
            CircuitState::Open => 0.3,
            CircuitState::HalfOpen => 0.15,
            CircuitState::Closed => 0.0,
        };

        (sr - latency_penalty + feedback_bonus - circuit_penalty).clamp(0.0, 1.0)
    }

    /// 获取熔断器的有效状态（考虑冷却期自动转换）
    ///
    /// 如果当前是 Open 且已过冷却期，返回 HalfOpen（允许试探）。
    pub fn effective_circuit_state(&self) -> CircuitState {
        match self.circuit_state {
            CircuitState::Open => {
                if self.circuit_opened_at > 0 {
                    let elapsed = now_secs().saturating_sub(self.circuit_opened_at);
                    if elapsed >= CIRCUIT_COOLDOWN_SECS {
                        return CircuitState::HalfOpen;
                    }
                }
                CircuitState::Open
            }
            other => other,
        }
    }

    /// 是否应该拒绝请求
    ///
    /// Closed / HalfOpen 允许请求，Open 拒绝请求。
    pub fn is_circuit_open(&self) -> bool {
        self.effective_circuit_state() == CircuitState::Open
    }

    /// 记录成功，更新熔断器状态
    fn circuit_on_success(&mut self) {
        match self.effective_circuit_state() {
            CircuitState::HalfOpen => {
                // 试探成功 → 恢复 Closed
                self.circuit_state = CircuitState::Closed;
                self.circuit_opened_at = 0;
                self.consecutive_failures = 0;
                info!("circuit breaker: HalfOpen → Closed (probe succeeded)");
            }
            _ => {
                self.consecutive_failures = 0;
                if self.circuit_state != CircuitState::Closed {
                    self.circuit_state = CircuitState::Closed;
                    self.circuit_opened_at = 0;
                }
            }
        }
    }

    /// 记录失败，更新熔断器状态
    fn circuit_on_failure(&mut self) {
        self.consecutive_failures += 1;
        match self.effective_circuit_state() {
            CircuitState::HalfOpen => {
                // 试探失败 → 重新 Open 并重置冷却期
                self.circuit_state = CircuitState::Open;
                self.circuit_opened_at = now_secs();
                info!("circuit breaker: HalfOpen → Open (probe failed, cooldown reset)");
            }
            CircuitState::Closed if self.consecutive_failures >= 3 => {
                // 连续失败达到阈值 → 打开熔断器
                self.circuit_state = CircuitState::Open;
                self.circuit_opened_at = now_secs();
                info!("circuit breaker: Closed → Open (consecutive_failures={})", self.consecutive_failures);
            }
            _ => {}
        }
    }
}

/// 学习引擎（线程安全，支持并发读写）
pub struct SkillLearner {
    /// capability -> SkillStats
    data: Mutex<HashMap<String, SkillStats>>,
    /// 持久化文件路径
    store_path: PathBuf,
}

impl SkillLearner {
    /// 从磁盘加载或创建新的学习引擎
    /// `learning_dir` 是学习数据目录（如 ~/.aion/learning）
    pub fn load(learning_dir: &Path) -> Self {
        let store_path = learning_dir.join("skill_stats.json");

        let data = if store_path.exists() {
            match fs::read_to_string(&store_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => HashMap::new(),
            }
        } else {
            HashMap::new()
        };

        Self {
            data: Mutex::new(data),
            store_path,
        }
    }

    /// 记录一次执行结果
    pub fn record(&self, capability: &str, success: bool, duration: Duration) {
        let mut data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let stats = data.entry(capability.to_string()).or_default();

        stats.total += 1;
        let ms = duration.as_millis() as u64;

        if success {
            stats.ok += 1;
            stats.circuit_on_success();
        } else {
            stats.fail += 1;
            stats.circuit_on_failure();
        }

        // 滑动平均延迟
        stats.avg_latency_ms =
            (stats.avg_latency_ms * (stats.total - 1) as f64 + ms as f64) / stats.total as f64;

        // 保留最近 20 条延迟记录
        stats.recent_latencies.push(ms);
        if stats.recent_latencies.len() > 20 {
            stats.recent_latencies.remove(0);
        }

        stats.last_used = now_secs();

        drop(data);

        // 异步持久化（不阻塞执行）
        let _ = self.persist();
    }

    /// 记录用户反馈
    pub fn record_feedback(&self, capability: &str, positive: bool) {
        let mut data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let stats = data.entry(capability.to_string()).or_default();
        if positive {
            stats.thumbs_up += 1;
        } else {
            stats.thumbs_down += 1;
        }
        drop(data);
        let _ = self.persist();
    }

    /// 获取某个能力的统计数据
    pub fn get_stats(&self, capability: &str) -> Option<SkillStats> {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        data.get(capability).cloned()
    }

    /// 获取全部统计数据（用于展示）
    pub fn all_stats(&self) -> HashMap<String, SkillStats> {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        data.clone()
    }

    /// 根据历史数据推荐最优能力（多个候选时选质量最高的）
    pub fn recommend(&self, candidates: &[String]) -> Option<String> {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        candidates
            .iter()
            .filter(|c| {
                // 排除熔断的能力
                data.get(c.as_str())
                    .map(|s| !s.is_circuit_open())
                    .unwrap_or(true)
            })
            .max_by(|a, b| {
                let sa = data.get(a.as_str()).map(|s| s.quality_score()).unwrap_or(0.5);
                let sb = data.get(b.as_str()).map(|s| s.quality_score()).unwrap_or(0.5);
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// 生成学习报告（JSON 格式）
    pub fn report(&self) -> serde_json::Value {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());

        let mut capabilities: Vec<_> = data.iter().collect();
        capabilities.sort_by(|a, b| b.1.total.cmp(&a.1.total));

        let entries: Vec<serde_json::Value> = capabilities
            .iter()
            .map(|(name, stats)| {
                serde_json::json!({
                    "capability": name,
                    "total": stats.total,
                    "success_rate": format!("{:.1}%", stats.success_rate() * 100.0),
                    "avg_latency_ms": format!("{:.0}", stats.avg_latency_ms),
                    "quality_score": format!("{:.2}", stats.quality_score()),
                    "circuit_state": format!("{:?}", stats.effective_circuit_state()),
                    "circuit_open": stats.is_circuit_open(),
                    "feedback": format!("+{} -{}", stats.thumbs_up, stats.thumbs_down),
                })
            })
            .collect();

        let total_executions: u64 = data.values().map(|s| s.total).sum();
        let total_ok: u64 = data.values().map(|s| s.ok).sum();

        serde_json::json!({
            "summary": {
                "total_executions": total_executions,
                "overall_success_rate": if total_executions > 0 {
                    format!("{:.1}%", total_ok as f64 / total_executions as f64 * 100.0)
                } else {
                    "N/A".to_string()
                },
                "capabilities_tracked": data.len(),
                "circuit_breakers_open": data.values().filter(|s| s.is_circuit_open()).count(),
            },
            "capabilities": entries,
        })
    }

    /// 持久化到磁盘
    fn persist(&self) -> Result<(), Box<dyn std::error::Error>> {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&*data)?;
        fs::write(&self.store_path, json)?;
        Ok(())
    }
}

/// 全局学习引擎实例
static LEARNER: std::sync::OnceLock<SkillLearner> = std::sync::OnceLock::new();

/// 初始化全局学习引擎
pub fn init_learner(workspace: &Path) {
    let _ = LEARNER.get_or_init(|| {
        // 优先使用用户目录下的固定路径，确保进程在任何 cwd 下都能写入
        let effective_path = std::env::var("AION_LEARNING_DIR")
            .map(PathBuf::from)
            .or_else(|_| {
                // Windows: USERPROFILE, Unix: HOME
                std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .map(|h| PathBuf::from(h).join(".aion").join("learning"))
            })
            .unwrap_or_else(|_| workspace.join("learning"));

        let learner = SkillLearner::load(&effective_path);
        info!("SkillLearner initialized at {:?}, tracking {} capabilities",
            effective_path, learner.all_stats().len());
        learner
    });
}

/// 获取全局学习引擎引用
pub fn learner() -> Option<&'static SkillLearner> {
    LEARNER.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stats(total: u64, ok: u64, fail: u64, latency: f64, consec_fail: u32) -> SkillStats {
        SkillStats {
            total, ok, fail, avg_latency_ms: latency,
            consecutive_failures: consec_fail,
            ..Default::default()
        }
    }

    #[test]
    fn test_success_rate_empty() {
        let s = SkillStats::default();
        assert_eq!(s.success_rate(), 0.5); // 未知给中等评分
    }

    #[test]
    fn test_success_rate_all_ok() {
        let s = make_stats(10, 10, 0, 100.0, 0);
        assert_eq!(s.success_rate(), 1.0);
    }

    #[test]
    fn test_success_rate_half() {
        let s = make_stats(10, 5, 5, 100.0, 0);
        assert_eq!(s.success_rate(), 0.5);
    }

    #[test]
    fn test_quality_score_perfect() {
        let s = make_stats(100, 100, 0, 500.0, 0);
        assert!(s.quality_score() > 0.9);
    }

    #[test]
    fn test_quality_score_high_latency_penalty() {
        let s = make_stats(100, 100, 0, 15000.0, 0);
        assert!(s.quality_score() < 1.0); // 高延迟应惩罚
    }

    #[test]
    fn test_circuit_breaker_closed() {
        let s = make_stats(10, 8, 2, 100.0, 2);
        assert!(!s.is_circuit_open());
    }

    #[test]
    fn test_circuit_breaker_open() {
        let mut s = make_stats(10, 7, 3, 100.0, 3);
        s.circuit_state = CircuitState::Open;
        s.circuit_opened_at = now_secs(); // 刚刚打开，还在冷却期内
        assert!(s.is_circuit_open());
    }

    #[test]
    fn test_quality_score_circuit_penalty() {
        let mut s = make_stats(10, 10, 0, 100.0, 3);
        s.circuit_state = CircuitState::Open;
        s.circuit_opened_at = now_secs();
        assert!(s.quality_score() < 0.8); // 熔断惩罚
    }

    #[test]
    fn test_circuit_halfopen_after_cooldown() {
        let mut s = make_stats(10, 7, 3, 100.0, 3);
        s.circuit_state = CircuitState::Open;
        // 模拟冷却期已过（设置为 6 分钟前）
        s.circuit_opened_at = now_secs().saturating_sub(360);
        assert_eq!(s.effective_circuit_state(), CircuitState::HalfOpen);
        assert!(!s.is_circuit_open()); // HalfOpen 允许试探请求
    }

    #[test]
    fn test_circuit_halfopen_probe_success() {
        let mut s = make_stats(10, 7, 3, 100.0, 3);
        s.circuit_state = CircuitState::Open;
        s.circuit_opened_at = now_secs().saturating_sub(360); // 冷却期已过
        assert_eq!(s.effective_circuit_state(), CircuitState::HalfOpen);

        // 试探成功 → Closed
        s.circuit_on_success();
        assert_eq!(s.circuit_state, CircuitState::Closed);
        assert_eq!(s.circuit_opened_at, 0);
        assert_eq!(s.consecutive_failures, 0);
    }

    #[test]
    fn test_circuit_halfopen_probe_failure() {
        let mut s = make_stats(10, 7, 3, 100.0, 3);
        s.circuit_state = CircuitState::Open;
        s.circuit_opened_at = now_secs().saturating_sub(360); // 冷却期已过
        assert_eq!(s.effective_circuit_state(), CircuitState::HalfOpen);

        // 试探失败 → 重新 Open，重置冷却期
        s.circuit_on_failure();
        assert_eq!(s.circuit_state, CircuitState::Open);
        assert!(s.circuit_opened_at > 0);
        // 冷却期刚重置，应该是 Open 而不是 HalfOpen
        assert_eq!(s.effective_circuit_state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_quality_score_halfopen_penalty() {
        let mut s = make_stats(100, 100, 0, 500.0, 3);
        s.circuit_state = CircuitState::Open;
        s.circuit_opened_at = now_secs().saturating_sub(360); // HalfOpen
        let score = s.quality_score();
        // HalfOpen 惩罚 0.15，小于 Open 的 0.3
        assert!(score > 0.8 && score < 1.0);
    }

    #[test]
    fn test_record_triggers_circuit_open() {
        let tmp = std::env::temp_dir().join("aion_test_circuit_open");
        let _ = std::fs::remove_dir_all(&tmp);
        let learner = SkillLearner::load(&tmp);

        // 3 次连续失败应触发熔断
        learner.record("fragile", false, Duration::from_millis(10));
        learner.record("fragile", false, Duration::from_millis(10));
        learner.record("fragile", false, Duration::from_millis(10));

        let stats = learner.get_stats("fragile").unwrap();
        assert_eq!(stats.circuit_state, CircuitState::Open);
        assert!(stats.circuit_opened_at > 0);
        assert!(stats.is_circuit_open());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_learner_record_and_recall() {
        let tmp = std::env::temp_dir().join("aion_test_learner");
        let _ = std::fs::remove_dir_all(&tmp);
        let learner = SkillLearner::load(&tmp);

        learner.record("test_cap", true, Duration::from_millis(50));
        learner.record("test_cap", true, Duration::from_millis(100));
        learner.record("test_cap", false, Duration::from_millis(200));

        let stats = learner.get_stats("test_cap").unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.ok, 2);
        assert_eq!(stats.fail, 1);
        assert_eq!(stats.consecutive_failures, 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_learner_recommend() {
        let tmp = std::env::temp_dir().join("aion_test_recommend");
        let _ = std::fs::remove_dir_all(&tmp);
        let learner = SkillLearner::load(&tmp);

        // good_cap: 100% 成功
        for _ in 0..5 {
            learner.record("good_cap", true, Duration::from_millis(10));
        }
        // bad_cap: 全部失败（熔断）
        for _ in 0..5 {
            learner.record("bad_cap", false, Duration::from_millis(10));
        }

        let candidates = vec!["good_cap".to_string(), "bad_cap".to_string()];
        let recommended = learner.recommend(&candidates);
        assert_eq!(recommended.as_deref(), Some("good_cap"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
