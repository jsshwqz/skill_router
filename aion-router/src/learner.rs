//! 技能学习引擎
//!
//! 持久化记录每次技能执行的成功率、延迟、使用频次，
//! 用于路由优化和技能自动进化。
//!
//! 数据存储在 `{workspace}/learning/skill_stats.json`。

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

use aion_intel::synth::Synthesizer;
use aion_types::types::RouterPaths;

use crate::error_kb::{ErrorKnowledgeBase, ErrorObservation, ErrorRecord, GateDecision, VerifiedFix};

/// 熔断器冷却期（秒）：Open 状态经过此时间后进入 HalfOpen
const CIRCUIT_COOLDOWN_SECS: u64 = 300; // 5 分钟

/// 自动进化触发阈值：连续失败达到此值触发 evolve
fn auto_evolve_threshold() -> u32 {
    std::env::var("AION_AUTO_EVOLVE_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
}

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

#[allow(clippy::derivable_impls)]
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

/// 是否启用执行遥测（默认启用）
fn telemetry_enabled() -> bool {
    match std::env::var("AION_TELEMETRY") {
        Ok(v) => !matches!(v.to_ascii_lowercase().as_str(), "off" | "0" | "false"),
        Err(_) => true,
    }
}

fn classify_error(error: &str) -> &'static str {
    let msg = error.to_ascii_lowercase();
    if msg.contains("timeout") || msg.contains("timed out") {
        "timeout"
    } else if msg.contains("auth") || msg.contains("unauthorized") || msg.contains("forbidden") {
        "auth_error"
    } else if msg.contains("security") || msg.contains("blocked") || msg.contains("deny") {
        "safety_block"
    } else if msg.contains("empty") || msg.contains("no output") {
        "empty_output"
    } else if msg.contains("not found") || msg.contains("missing") {
        "not_found"
    } else {
        "runtime_error"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    #[serde(default)]
    pub timestamp: u64,
    pub capability: String,
    #[serde(default)]
    pub skill: String,
    #[serde(default)]
    pub source: String,
    pub success: bool,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub error_class: String,
    #[serde(default)]
    pub empty_output: bool,
    /// 事件类型：execution（默认）| change | decision
    #[serde(default)]
    pub event_type: String,
    /// 事件摘要（用于 change/decision 类型）
    #[serde(default)]
    pub summary: String,
    /// 事件详情（用于 change/decision 类型）
    #[serde(default)]
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutonomyPolicy {
    #[serde(default)]
    pub blocked_capabilities: Vec<String>,
    #[serde(default)]
    pub preferred_capabilities: Vec<String>,
    #[serde(default)]
    pub recent_success_rate: f64,
    #[serde(default)]
    pub unresolved_failures: usize,
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
                info!(
                    "circuit breaker: Closed → Open (consecutive_failures={})",
                    self.consecutive_failures
                );
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
    /// 执行事件日志（JSONL）
    events_path: PathBuf,
    /// 工作区根目录（用于生成 evolved 技能）
    workspace_root: PathBuf,
    /// Durable error knowledge base.
    error_kb: Mutex<ErrorKnowledgeBase>,
    /// Error knowledge base persistence path.
    error_kb_path: PathBuf,
}

impl SkillLearner {
    /// 从磁盘加载或创建新的学习引擎
    /// `learning_dir` 是学习数据目录（如 ~/.aion/learning）
    /// `workspace_root` 是项目根目录（用于生成 evolved 技能）
    pub fn load(learning_dir: &Path, workspace_root: &Path) -> Self {
        let store_path = learning_dir.join("skill_stats.json");
        let events_path = learning_dir.join("execution_events.jsonl");
        let error_kb_path = learning_dir.join("error_knowledge.json");

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
            events_path,
            workspace_root: workspace_root.to_path_buf(),
            error_kb: Mutex::new(ErrorKnowledgeBase::load(&error_kb_path).unwrap_or_default()),
            error_kb_path,
        }
    }

    /// Record a durable error observation and return its stable fingerprint.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_error(
        &self,
        capability: &str,
        error_class: &str,
        error: &str,
        context: &serde_json::Value,
        version: &str,
        observed_at: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut knowledge = self.error_kb.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let fingerprint = knowledge.observe(ErrorObservation {
            capability: capability.to_string(),
            error_class: error_class.to_string(),
            error: error.to_string(),
            context: context.clone(),
            version: version.to_string(),
            observed_at,
        });
        knowledge.save(&self.error_kb_path)?;
        Ok(fingerprint)
    }

    /// Classify and persist an execution failure using its full context.
    pub fn observe_execution_error(
        &self,
        capability: &str,
        error: &str,
        context: &serde_json::Value,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.observe_error(
            capability,
            classify_error(error),
            error,
            context,
            env!("CARGO_PKG_VERSION"),
            now_secs(),
        )
    }

    /// Check known errors before executing a capability with the same context.
    pub fn pre_execution_gate(&self, capability: &str, context: &serde_json::Value) -> GateDecision {
        self.error_kb
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pre_execution(capability, context)
    }

    /// Return one durable error record.
    pub fn error_record(&self, fingerprint: &str) -> Option<ErrorRecord> {
        self.error_kb
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(fingerprint)
            .cloned()
    }

    /// Resolve exactly one fingerprint after a successful verified replay.
    pub fn resolve_error_fingerprint(
        &self,
        fingerprint: &str,
        version: &str,
        timestamp: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut knowledge = self.error_kb.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        knowledge.resolve_fingerprint_success(fingerprint, version, timestamp)?;
        knowledge.save(&self.error_kb_path)?;
        Ok(())
    }

    /// Mark an error fingerprint reproducible and persist it.
    pub fn mark_error_reproduced(&self, fingerprint: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut knowledge = self.error_kb.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        knowledge.mark_reproduced(fingerprint)?;
        knowledge.save(&self.error_kb_path)?;
        Ok(())
    }

    /// Attach verified fix evidence and persist it.
    pub fn mark_error_fixed(&self, fingerprint: &str, fix: VerifiedFix) -> Result<(), Box<dyn std::error::Error>> {
        let mut knowledge = self.error_kb.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        knowledge.mark_fixed(fingerprint, fix)?;
        knowledge.save(&self.error_kb_path)?;
        Ok(())
    }

    /// Mark a fixed fingerprint verified for a version and persist it.
    pub fn mark_error_verified(
        &self,
        fingerprint: &str,
        version: &str,
        timestamp: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut knowledge = self.error_kb.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        knowledge.mark_verified(fingerprint, version, timestamp)?;
        knowledge.save(&self.error_kb_path)?;
        Ok(())
    }

    /// 记录一次执行结果
    pub fn record(&self, capability: &str, success: bool, duration: Duration) {
        self.record_execution(capability, capability, "unknown", success, duration, None, false);
    }

    /// 记录一次完整执行（含来源、错误分类、空输出标记）
    #[allow(clippy::too_many_arguments)]
    pub fn record_execution(
        &self,
        capability: &str,
        skill: &str,
        source: &str,
        success: bool,
        duration: Duration,
        error: Option<&str>,
        empty_output: bool,
    ) {
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
            // 触发自动进化检查（异步，不阻塞）
            let cap = capability.to_string();
            let consec = stats.consecutive_failures;
            let threshold = auto_evolve_threshold();
            if consec == threshold {
                let events = self.events_path.clone();
                let ws = self.workspace_root.clone();
                let error_summary = Self::record_auto_evolve_event(&events, &cap, consec);
                let _ = std::thread::spawn(move || {
                    Self::generate_evolved_skill(&ws, &cap, consec, &error_summary);
                });
            }
        }

        // 滑动平均延迟
        stats.avg_latency_ms = (stats.avg_latency_ms * (stats.total - 1) as f64 + ms as f64) / stats.total as f64;

        // 保留最近 20 条延迟记录
        stats.recent_latencies.push(ms);
        if stats.recent_latencies.len() > 20 {
            stats.recent_latencies.remove(0);
        }

        stats.last_used = now_secs();

        drop(data);

        // 异步持久化（不阻塞执行）
        let _ = self.persist();

        if telemetry_enabled() {
            let event = ExecutionEvent {
                timestamp: now_secs(),
                capability: capability.to_string(),
                skill: skill.to_string(),
                source: source.to_string(),
                success,
                duration_ms: ms,
                error_class: if success {
                    String::new()
                } else {
                    error.map(classify_error).unwrap_or("runtime_error").to_string()
                },
                empty_output,
                event_type: String::new(),
                summary: String::new(),
                details: String::new(),
            };
            let _ = self.append_event(&event);
        }
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

    /// 记录代码变更事件
    pub fn record_change(&self, kind: &str, file: &str, summary: &str) {
        if !telemetry_enabled() {
            return;
        }
        let event = ExecutionEvent {
            timestamp: now_secs(),
            capability: kind.to_string(),
            skill: "change".to_string(),
            source: "session".to_string(),
            success: true,
            duration_ms: 0,
            error_class: String::new(),
            empty_output: false,
            event_type: "change".to_string(),
            summary: summary.to_string(),
            details: file.to_string(),
        };
        let _ = self.append_event(&event);
    }

    /// 记录架构/设计决策事件
    pub fn record_decision(&self, context: &str, choice: &str, rationale: &str) {
        if !telemetry_enabled() {
            return;
        }
        let event = ExecutionEvent {
            timestamp: now_secs(),
            capability: "decision".to_string(),
            skill: "decision".to_string(),
            source: "session".to_string(),
            success: true,
            duration_ms: 0,
            error_class: String::new(),
            empty_output: false,
            event_type: "decision".to_string(),
            summary: format!("{} → {}", context, choice),
            details: rationale.to_string(),
        };
        let _ = self.append_event(&event);
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
                data.get(c.as_str()).map(|s| !s.is_circuit_open()).unwrap_or(true)
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
        capabilities.sort_by_key(|entry| std::cmp::Reverse(entry.1.total));

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
            "evolution": self.evolution_report(10),
        })
    }

    /// 生成自进化报告（基于 execution_events.jsonl）
    pub fn evolution_report(&self, latest_limit: usize) -> serde_json::Value {
        let events = self.read_events();
        if events.is_empty() {
            return serde_json::json!({
                "summary": {
                    "total_events": 0,
                    "success_rate": "N/A"
                },
                "sources": {},
                "errors": {},
                "latest_failures": [],
                "recommendations": ["暂未采集到调用事件，先执行常用能力以建立基线"]
            });
        }

        let total = events.len() as u64;
        let ok = events.iter().filter(|e| e.success).count() as u64;

        let mut source_map: HashMap<String, u64> = HashMap::new();
        let mut error_map: HashMap<String, u64> = HashMap::new();
        for e in &events {
            *source_map.entry(e.source.clone()).or_insert(0) += 1;
            if !e.success && !e.error_class.is_empty() {
                *error_map.entry(e.error_class.clone()).or_insert(0) += 1;
            }
        }

        let latest_failures: Vec<_> = events
            .iter()
            .rev()
            .filter(|e| !e.success)
            .take(latest_limit)
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "capability": e.capability,
                    "skill": e.skill,
                    "source": e.source,
                    "duration_ms": e.duration_ms,
                    "error_class": e.error_class,
                    "empty_output": e.empty_output,
                })
            })
            .collect();

        // 失败是否“已修复”判定：
        // 从最新往回扫描，若某能力在失败之后出现成功，则认为历史失败已修复。
        let mut seen_success: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut unresolved_failures: Vec<&ExecutionEvent> = Vec::new();
        for e in events.iter().rev() {
            if e.success {
                seen_success.insert(e.capability.clone());
            } else if !seen_success.contains(&e.capability) {
                unresolved_failures.push(e);
            }
        }
        unresolved_failures.reverse();
        let unresolved_failures_json: Vec<_> = unresolved_failures
            .iter()
            .rev()
            .take(latest_limit)
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "capability": e.capability,
                    "skill": e.skill,
                    "source": e.source,
                    "duration_ms": e.duration_ms,
                    "error_class": e.error_class,
                    "empty_output": e.empty_output,
                })
            })
            .collect();

        // 最近窗口（默认 50 次）指标：更反映当前状态，避免老故障长期污染结论。
        let recent_window = events.iter().rev().take(50).collect::<Vec<_>>();
        let recent_total = recent_window.len() as u64;
        let recent_ok = recent_window.iter().filter(|e| e.success).count() as u64;
        let recent_failed = recent_total.saturating_sub(recent_ok);

        let success_rate = ok as f64 / total as f64;
        let failed = total.saturating_sub(ok);
        let mut recommendations = Vec::new();
        if !unresolved_failures.is_empty() {
            recommendations.push("检测到未修复失败，建议优先处理 unresolved_failures 中的能力并回归验证");
        }
        if failed > 0 && unresolved_failures.is_empty() {
            recommendations.push("存在历史失败但已被后续成功覆盖，建议继续观察近期窗口指标");
        }
        if success_rate < 0.95 {
            recommendations.push("整体成功率低于 95%，建议收敛高成功率能力白名单并降低任务复杂度");
        }
        if recent_total >= 10 && recent_failed > 0 {
            recommendations.push("最近 50 次调用中仍有失败，建议优先修复近期重复失败项");
        }
        if success_rate < 0.8 {
            recommendations.push("整体成功率偏低，建议先收敛到高成功率能力白名单");
        }
        if error_map.get("runtime_error").copied().unwrap_or(0) > 0 {
            recommendations.push("runtime_error 存在，建议补输入兼容与参数校验，减少调用契约错误");
        }
        if error_map.get("not_found").copied().unwrap_or(0) > 0 {
            recommendations.push("not_found 偏多，建议检查能力注册和路由规则覆盖");
        }
        if error_map.get("timeout").copied().unwrap_or(0) > 0 {
            recommendations.push("timeout 偏多，建议缩短单次任务并降低并发/引擎数量");
        }
        if error_map.get("safety_block").copied().unwrap_or(0) > 0 {
            recommendations.push("存在安全拦截，建议检查输入是否触发高风险规则");
        }
        if error_map.get("empty_output").copied().unwrap_or(0) > 0 {
            recommendations.push("出现空输出，建议增加重试策略或切换稳定模型");
        }
        if recommendations.is_empty() {
            recommendations.push("当前运行稳定，可逐步扩大能力覆盖面");
        }

        serde_json::json!({
            "summary": {
                "total_events": total,
                "success_events": ok,
                "failed_events": total.saturating_sub(ok),
                "success_rate": format!("{:.1}%", success_rate * 100.0),
                "recent_window": recent_total,
                "recent_success_rate": if recent_total > 0 {
                    format!("{:.1}%", recent_ok as f64 / recent_total as f64 * 100.0)
                } else {
                    "N/A".to_string()
                },
                "unresolved_failures": unresolved_failures.len(),
            },
            "sources": source_map,
            "errors": error_map,
            "latest_failures": latest_failures,
            "unresolved_failures": unresolved_failures_json,
            "recommendations": recommendations,
        })
    }

    /// 生成会话报告：汇总本次 session 的变更、决策、错误和推荐
    pub fn session_report(&self) -> serde_json::Value {
        let events = self.read_events();
        if events.is_empty() {
            return json!({
                "summary": "暂无事件数据",
                "changes": [],
                "decisions": [],
                "execution_stats": {},
                "recommendations": ["执行一些能力后再查看会话报告"]
            });
        }

        // 按事件类型分类
        let changes: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "change")
            .map(|e| {
                json!({
                    "type": e.capability,
                    "file": e.details,
                    "summary": e.summary,
                    "timestamp": e.timestamp,
                })
            })
            .collect();

        let decisions: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == "decision")
            .map(|e| {
                json!({
                    "context_choice": e.summary,
                    "rationale": e.details,
                    "timestamp": e.timestamp,
                })
            })
            .collect();

        let executions: Vec<_> = events
            .iter()
            .filter(|e| e.event_type.is_empty() || e.event_type == "execution")
            .collect();
        let total_exec = executions.len();
        let ok_exec = executions.iter().filter(|e| e.success).count();

        // 最近失败
        let recent_failures: Vec<_> = executions
            .iter()
            .rev()
            .filter(|e| !e.success)
            .take(5)
            .map(|e| {
                json!({
                    "capability": e.capability,
                    "error": e.error_class,
                    "timestamp": e.timestamp,
                })
            })
            .collect();

        // 反复出现的错误模式（最近 100 次执行中，同 error_class 出现 >=2 次）
        let recent_100 = executions.iter().rev().take(100).collect::<Vec<_>>();
        let mut pattern_map: std::collections::BTreeMap<String, (u32, u64, Vec<String>)> =
            std::collections::BTreeMap::new();
        for e in recent_100.iter().filter(|e| !e.success && !e.error_class.is_empty()) {
            let key = format!("{}/{}", e.capability, e.error_class);
            let entry = pattern_map.entry(key).or_insert((0, 0, Vec::new()));
            entry.0 += 1;
            entry.1 = entry.1.max(e.timestamp);
            entry.2.push(e.capability.clone());
        }
        let recurring_patterns: Vec<_> = pattern_map
            .iter()
            .filter(|(_, (count, _, _))| *count >= 2)
            .map(|(key, (count, latest_ts, _caps))| {
                let parts: Vec<&str> = key.splitn(2, '/').collect();
                json!({
                    "capability": parts.first().unwrap_or(&"unknown"),
                    "error_class": parts.get(1).unwrap_or(&"unknown"),
                    "occurrences": *count,
                    "latest": *latest_ts,
                    "severity": if *count >= 5 { "high" } else if *count >= 3 { "medium" } else { "low" },
                })
            })
            .collect();

        // 推荐
        let mut recommendations: Vec<String> = Vec::new();
        if !recent_failures.is_empty() {
            recommendations.push("存在未修复失败，建议优先处理".to_string());
        }
        for p in &recurring_patterns {
            let cap = p["capability"].as_str().unwrap_or("");
            let cls = p["error_class"].as_str().unwrap_or("");
            let occ = p["occurrences"].as_u64().unwrap_or(0);
            let sev = p["severity"].as_str().unwrap_or("low");
            recommendations.push(format!("[{}] {} 的 {} 已出现 {} 次，建议排查根因", sev, cap, cls, occ));
        }
        if total_exec > 0 && (ok_exec as f64 / total_exec as f64) < 0.9 {
            recommendations.push("成功率偏低，建议排查失败原因".to_string());
        }
        if !recurring_patterns.is_empty() {
            recommendations.push("发现反复错误模式，修复后请用 record_change 记录改动".to_string());
        }
        if changes.is_empty() && decisions.is_empty() {
            recommendations.push("本次无记录变更和决策，考虑调用 record_change / record_decision".to_string());
        }
        if recommendations.is_empty() {
            recommendations.push("运行稳定，可继续扩展能力".to_string());
        }

        json!({
            "changes": changes,
            "decisions": decisions,
            "execution_stats": {
                "total": total_exec,
                "succeeded": ok_exec,
                "failed": total_exec.saturating_sub(ok_exec),
                "success_rate": if total_exec > 0 {
                    format!("{:.1}%", ok_exec as f64 / total_exec as f64 * 100.0)
                } else { "N/A".to_string() },
            },
            "recent_failures": recent_failures,
            "recurring_patterns": recurring_patterns,
            "recommendations": recommendations,
        })
    }

    /// 生成可执行的自治策略（供路由前注入）。
    pub fn autonomy_policy(&self) -> AutonomyPolicy {
        let events = self.read_events();
        if events.is_empty() {
            return AutonomyPolicy::default();
        }

        // 计算 unresolved failures（同 evolution_report 逻辑）
        let mut seen_success: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut unresolved_failures: Vec<&ExecutionEvent> = Vec::new();
        for e in events.iter().rev() {
            if e.success {
                seen_success.insert(e.capability.clone());
            } else if !seen_success.contains(&e.capability) {
                unresolved_failures.push(e);
            }
        }

        let blocked_capabilities: std::collections::HashSet<String> = unresolved_failures
            .iter()
            .filter(|e| matches!(e.error_class.as_str(), "runtime_error" | "timeout" | "safety_block"))
            .map(|e| e.capability.clone())
            .collect();

        // 最近窗口成功率
        let recent_window = events.iter().rev().take(50).collect::<Vec<_>>();
        let recent_total = recent_window.len();
        let recent_ok = recent_window.iter().filter(|e| e.success).count();
        let recent_success_rate = if recent_total > 0 {
            recent_ok as f64 / recent_total as f64
        } else {
            0.0
        };

        // 优先能力：高质量且未熔断的前 5 个
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let mut ranked: Vec<(String, f64)> = data
            .iter()
            .filter(|(_, s)| !s.is_circuit_open())
            .map(|(cap, s)| (cap.clone(), s.quality_score()))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let preferred_capabilities = ranked
            .into_iter()
            .filter(|(_, score)| *score >= 0.8)
            .take(5)
            .map(|(cap, _)| cap)
            .collect::<Vec<_>>();

        AutonomyPolicy {
            blocked_capabilities: blocked_capabilities.into_iter().collect(),
            preferred_capabilities,
            recent_success_rate,
            unresolved_failures: unresolved_failures.len(),
        }
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

    fn append_event(&self, event: &ExecutionEvent) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.events_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&self.events_path)?;
        writeln!(file, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }

    /// 读取指定能力的最近 N 条失败事件
    #[allow(dead_code)]
    fn read_recent_failures(&self, capability: &str, limit: usize) -> Vec<ExecutionEvent> {
        self.read_events()
            .into_iter()
            .filter(|e| e.capability == capability && !e.success)
            .rev()
            .take(limit)
            .collect()
    }

    /// 记录自动进化触发事件，并返回用于生成技能的错误摘要。
    fn record_auto_evolve_event(events_path: &Path, capability: &str, consecutive_failures: u32) -> String {
        // 读取最近的失败事件以获取错误原因
        let events = Self::read_events_from(events_path);
        let failures: Vec<&str> = events
            .iter()
            .filter(|e| e.capability == capability && !e.success)
            .rev()
            .take(5)
            .map(|e| e.error_class.as_str())
            .filter(|c| !c.is_empty())
            .collect();

        let error_summary = if failures.is_empty() {
            "unknown".to_string()
        } else {
            let mut seen = Vec::new();
            for c in &failures {
                if !seen.contains(c) {
                    seen.push(*c);
                }
            }
            seen.join(", ")
        };

        // 追加一个 evolve 事件
        let event = ExecutionEvent {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            capability: capability.to_string(),
            skill: String::new(),
            source: String::new(),
            success: false,
            duration_ms: 0,
            error_class: error_summary.clone(),
            empty_output: false,
            event_type: "evolve".to_string(),
            summary: format!(
                "auto_evolve: consecutive_failures={} errors=[{}]",
                consecutive_failures, error_summary
            ),
            details: String::new(),
        };

        if let Some(parent) = events_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(events_path) {
            let _ = writeln!(file, "{}", serde_json::to_string(&event).unwrap_or_default());
        }

        info!(
            "auto_evolve: capability={} consecutive_failures={} errors=[{}]",
            capability, consecutive_failures, error_summary
        );

        error_summary
    }

    /// 在后台生成 evolved 技能，避免阻塞执行记录。
    fn generate_evolved_skill(workspace_root: &Path, capability: &str, consecutive_failures: u32, error_summary: &str) {
        let paths = RouterPaths::for_workspace(workspace_root);
        let _ = Synthesizer::evolve_with_failures(
            &paths,
            capability,
            capability,
            "",
            &format!(
                "consecutive_failures={} error_types=[{}]",
                consecutive_failures, error_summary
            ),
        );
    }

    fn read_events_from(path: &Path) -> Vec<ExecutionEvent> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);
        reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<ExecutionEvent>(&line).ok())
            .collect()
    }
}

impl SkillLearner {
    fn read_events(&self) -> Vec<ExecutionEvent> {
        Self::read_events_from(&self.events_path)
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

        let learner = SkillLearner::load(&effective_path, workspace);
        info!(
            "SkillLearner initialized at {:?}, tracking {} capabilities",
            effective_path,
            learner.all_stats().len()
        );
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
            total,
            ok,
            fail,
            avg_latency_ms: latency,
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
        let learner = SkillLearner::load(&tmp, &tmp);

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
        let learner = SkillLearner::load(&tmp, &tmp);

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
        let learner = SkillLearner::load(&tmp, &tmp);

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

    #[test]
    fn test_autonomy_policy_blocks_unresolved_runtime_error() {
        let tmp = std::env::temp_dir().join("aion_test_autonomy_policy");
        let _ = std::fs::remove_dir_all(&tmp);
        let learner = SkillLearner::load(&tmp, &tmp);

        // 先失败且未恢复，应进入 blocked_capabilities
        learner.record_execution(
            "text_wordcount",
            "text_wordcount_placeholder",
            "cli",
            false,
            Duration::from_millis(1),
            Some("context.text is required"),
            false,
        );

        let policy = learner.autonomy_policy();
        assert!(policy.blocked_capabilities.contains(&"text_wordcount".to_string()));
        assert!(policy.unresolved_failures >= 1);

        // 恢复成功后不应再 block
        learner.record_execution(
            "text_wordcount",
            "text_wordcount_placeholder",
            "cli",
            true,
            Duration::from_millis(1),
            None,
            false,
        );
        let policy2 = learner.autonomy_policy();
        assert!(!policy2.blocked_capabilities.contains(&"text_wordcount".to_string()));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_auto_evolve_on_3_failures() {
        let tmp = std::env::temp_dir().join(format!("aion_test_auto_evolve_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&tmp);
        // 记录：3 次失败 → 应触发 auto_evolve 事件
        let learner = SkillLearner::load(&tmp, &tmp);
        for _i in 0..3 {
            learner.record_execution(
                "test_fragile",
                "test_skill",
                "test",
                false,
                Duration::from_millis(1),
                Some("timeout"),
                false,
            );
            // 等待异步事件写入
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // 检查 stats
        let stats = learner.get_stats("test_fragile").unwrap();
        assert!(
            stats.circuit_state == CircuitState::Open,
            "circuit should be open after 3 failures"
        );
        // 等待异步线程写入 evolve 事件
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let events = loop {
            let events = SkillLearner::read_events_from(&learner.events_path);
            if events.iter().any(|event| event.event_type == "evolve") || std::time::Instant::now() >= deadline {
                break events;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        // 检查 evolve 事件
        let evolve_events: Vec<&ExecutionEvent> = events.iter().filter(|e| e.event_type == "evolve").collect();
        assert!(!evolve_events.is_empty(), "expected at least one evolve event");
        assert!(evolve_events[0].capability == "test_fragile");
        info!("auto-evolve verified: event_type=evolve capability=test_fragile");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
