//! 执行指标记录
//!
//! 使用 `metrics` crate 记录技能执行计数器和耗时直方图。
//! 指标名称遵循 Prometheus 命名规范。

use std::time::Duration;

use aion_types::types::TokenUsage;

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

/// 记录 AI 调用的 Token 消耗
///
/// 指标：
/// - skill_ai_prompt_tokens_total: 输入 token 累计
/// - skill_ai_completion_tokens_total: 输出 token 累计
/// - skill_ai_total_tokens_total: 总 token 累计
/// - skill_ai_cached_tokens_total: 缓存命中 token（Anthropic prompt caching）
pub fn record_token_usage(
    skill_name: &str,
    capability: &str,
    provider: &str,
    usage: &TokenUsage,
) {
    metrics::counter!(
        "skill_ai_prompt_tokens_total",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string(),
        "provider" => provider.to_string()
    )
    .increment(usage.prompt_tokens as u64);

    metrics::counter!(
        "skill_ai_completion_tokens_total",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string(),
        "provider" => provider.to_string()
    )
    .increment(usage.completion_tokens as u64);

    metrics::counter!(
        "skill_ai_total_tokens_total",
        "skill" => skill_name.to_string(),
        "capability" => capability.to_string(),
        "provider" => provider.to_string()
    )
    .increment(usage.total_tokens as u64);

    if usage.cached_tokens > 0 {
        metrics::counter!(
            "skill_ai_cached_tokens_total",
            "skill" => skill_name.to_string(),
            "capability" => capability.to_string(),
            "provider" => provider.to_string()
        )
        .increment(usage.cached_tokens as u64);
    }
}
