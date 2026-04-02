//! Health Check builtin skill
//!
//! Reads `.skill-router/cli_health.json` for per-engine health status
//! and `VERSION.json` (workspace root) for server version information.
//! Returns a unified health report covering all AI engines and the server itself.

use anyhow::Result;
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

/// HealthCheck: reads cli_health.json and VERSION.json, returns unified status
pub struct HealthCheck;

#[async_trait::async_trait]
impl BuiltinSkill for HealthCheck {
    fn name(&self) -> &'static str {
        "health_check"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        // Determine workspace root from context or fall back to current dir
        let workspace_root = context
            .context
            .get("workspace")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let state_dir = workspace_root.join(".skill-router");

        // ── Read cli_health.json ────────────────────────────────────────
        let health_path = state_dir.join("cli_health.json");
        let engines = if health_path.exists() {
            match std::fs::read_to_string(&health_path) {
                Ok(content) => match serde_json::from_str::<Value>(&content) {
                    Ok(parsed) => parsed.get("engines").cloned().unwrap_or(Value::Null),
                    Err(e) => json!({ "error": format!("failed to parse cli_health.json: {}", e) }),
                },
                Err(e) => json!({ "error": format!("failed to read cli_health.json: {}", e) }),
            }
        } else {
            json!({ "error": "cli_health.json not found" })
        };

        // ── Extract per-engine status ───────────────────────────────────
        let engine_names = ["claude", "openai", "gemini"];
        let mut engine_statuses = serde_json::Map::new();
        let mut all_healthy = true;

        for name in &engine_names {
            if let Some(engine_data) = engines.get(name) {
                let status = engine_data
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let successes = engine_data.get("successes").and_then(|v| v.as_u64()).unwrap_or(0);
                let failures = engine_data.get("failures").and_then(|v| v.as_u64()).unwrap_or(0);
                let consecutive_failures = engine_data
                    .get("consecutive_failures")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let avg_latency_ms = engine_data
                    .get("avg_latency_ms")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let last_error_kind = engine_data.get("last_error_kind").cloned().unwrap_or(Value::Null);
                let cooldown_until = engine_data.get("cooldown_until").cloned().unwrap_or(Value::Null);

                if status != "healthy" {
                    all_healthy = false;
                }

                engine_statuses.insert(
                    name.to_string(),
                    json!({
                        "status": status,
                        "successes": successes,
                        "failures": failures,
                        "consecutive_failures": consecutive_failures,
                        "avg_latency_ms": avg_latency_ms,
                        "last_error_kind": last_error_kind,
                        "cooldown_until": cooldown_until,
                    }),
                );
            } else {
                all_healthy = false;
                engine_statuses.insert(
                    name.to_string(),
                    json!({ "status": "not_configured" }),
                );
            }
        }

        // ── Read VERSION.json ───────────────────────────────────────────
        let version_path = workspace_root.join("VERSION.json");
        let server_version = if version_path.exists() {
            match std::fs::read_to_string(&version_path) {
                Ok(content) => match serde_json::from_str::<Value>(&content) {
                    Ok(parsed) => parsed,
                    Err(e) => json!({ "error": format!("failed to parse VERSION.json: {}", e) }),
                },
                Err(e) => json!({ "error": format!("failed to read VERSION.json: {}", e) }),
            }
        } else {
            // Fall back to Cargo package version
            json!({
                "version": env!("CARGO_PKG_VERSION"),
                "source": "cargo_pkg"
            })
        };

        // ── Build overall status ────────────────────────────────────────
        let overall_status = if all_healthy { "healthy" } else { "degraded" };

        Ok(json!({
            "overall_status": overall_status,
            "engines": Value::Object(engine_statuses),
            "server_version": server_version,
            "health_file": health_path.to_string_lossy(),
            "timestamp": super::now_epoch_ms(),
        }))
    }
}
