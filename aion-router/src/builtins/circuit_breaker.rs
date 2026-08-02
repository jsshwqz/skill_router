use anyhow::Result;
use serde_json::{json, Value};

use super::super::circuit_breaker::{BreakerConfig, CircuitBreaker};
use aion_types::types::{ExecutionContext, SkillDefinition};

use super::BuiltinSkill;

pub struct McpCircuitBreaker;

#[async_trait::async_trait]
impl BuiltinSkill for McpCircuitBreaker {
    fn name(&self) -> &'static str {
        "mcp_circuit_breaker"
    }

    async fn execute(
        &self,
        _skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let workspace_root = context
            .context
            .get("workspace")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let state_dir = workspace_root.join(".skill-router");
        let mut cb = CircuitBreaker::new(BreakerConfig::default(), &state_dir);

        match context.context.get("action").and_then(|v| v.as_str()) {
            Some("status") => Ok(cb.status_report()),
            Some("reset") => {
                cb.reset();
                Ok(json!({"status": "reset", "message": "Circuit breaker reset"}))
            }
            Some("allow") => Ok(json!({"allowed": cb.allow_call()})),
            Some("success") => {
                cb.record_success();
                Ok(json!({"status": "success_recorded"}))
            }
            Some("failure") => {
                cb.record_failure();
                Ok(json!({"status": "failure_recorded"}))
            }
            _ => Ok(cb.status_report()),
        }
    }
}
