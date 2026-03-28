//! 执行指标记录
//!
//! 使用 `metrics` crate 记录技能执行计数器和耗时直方图。
//! 指标名称遵循 Prometheus 命名规范。

use std::time::Duration;

/// 记录一次技能执行的指标
pub fn record_skill_execution(
    skill_name: &str,
    capability: &str,
    success: bool,
    duration: Duration,
) {
    let status = if success { "ok" } else { "error" };

    metrics::counter!(
        "skill_executions_total",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string(),
        "status" => status.to_string()
    )
    .increment(1);

    metrics::histogram!(
        "skill_execution_duration_seconds",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string()
    )
    .record(duration.as_secs_f64());
}
