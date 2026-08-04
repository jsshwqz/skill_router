use anyhow::Result;
use serde_json::{json, Value};

use super::super::engine_health::HealthManager;
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

pub struct EngineHealthCheck;

#[async_trait::async_trait]
impl BuiltinSkill for EngineHealthCheck {
    fn name(&self) -> &'static str {
        "engine_health_check"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let workspace_root = context
            .context
            .get("workspace")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let state_dir = workspace_root.join(".skill-router");
        let mut mgr = HealthManager::new(&state_dir);

        match context.context.get("action").and_then(|v| v.as_str()) {
            Some("record_success") => {
                let engine = context
                    .context
                    .get("engine")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                mgr.record_success(engine);
                Ok(json!({"status": "success_recorded", "engine": engine}))
            }
            Some("record_failure") => {
                let engine = context
                    .context
                    .get("engine")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let error = context
                    .context
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                mgr.record_failure(engine, error, 0.0);
                Ok(json!({"status": "failure_recorded", "engine": engine}))
            }
            Some("status") => {
                let engine = context
                    .context
                    .get("engine")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                if let Some(s) = mgr.get_status(engine) {
                    Ok(json!({
                        "engine": engine,
                        "healthy": s.is_healthy(),
                        "degraded": s.is_degraded(),
                        "unhealthy": s.is_unhealthy(),
                        "consecutive_failures": s.consecutive_failures,
                        "total_failures": s.total_failures,
                        "total_successes": s.total_successes,
                    }))
                } else {
                    Ok(json!({"engine": engine, "state": "unknown"}))
                }
            }
            _ => Ok(mgr.health_report()),
        }
    }
}
