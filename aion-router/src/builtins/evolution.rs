use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use crate::evolution::{EvolutionRequest, EvolutionRunner, GateSpec, PatchCandidate};

use super::BuiltinSkill;

/// Executes generated Rust patch candidates in isolated worktrees and selects a non-regressing winner.
pub struct EvolutionRun;

#[async_trait::async_trait]
impl BuiltinSkill for EvolutionRun {
    fn name(&self) -> &'static str {
        "evolution_run"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let request: EvolutionRequest = serde_json::from_value(
            context
                .context
                .get("request")
                .cloned()
                .ok_or_else(|| anyhow!("evolution_run requires request"))?,
        )?;
        let candidates: Vec<PatchCandidate> = serde_json::from_value(
            context
                .context
                .get("candidates")
                .cloned()
                .ok_or_else(|| anyhow!("evolution_run requires candidates"))?,
        )?;
        let gates: Vec<GateSpec> = serde_json::from_value(
            context
                .context
                .get("gates")
                .cloned()
                .ok_or_else(|| anyhow!("evolution_run requires gates"))?,
        )?;
        let outcome = EvolutionRunner::run(&request, &candidates, &gates)?;
        Ok(json!(outcome))
    }
}
