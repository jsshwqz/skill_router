use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use aion_types::types::{ExecutionContext, SkillDefinition};

use crate::error_kb::VerifiedFix;

use super::BuiltinSkill;

/// Manage durable error lifecycle records used by self-evolution.
pub struct ErrorKnowledge;

#[async_trait::async_trait]
impl BuiltinSkill for ErrorKnowledge {
    fn name(&self) -> &'static str {
        "error_knowledge"
    }

    async fn execute(&self, _skill: &SkillDefinition, context: &ExecutionContext) -> Result<Value> {
        let learner = crate::learner::learner().ok_or_else(|| anyhow!("learning engine is not initialized"))?;
        let action = context
            .context
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("error_knowledge requires action"))?;
        let fingerprint = context
            .context
            .get("fingerprint")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match action {
            "get" => Ok(json!({"record": learner.error_record(fingerprint)})),
            "reproduced" => {
                learner
                    .mark_error_reproduced(fingerprint)
                    .map_err(|error| anyhow!(error.to_string()))?;
                Ok(json!({"status":"reproduced","fingerprint":fingerprint}))
            }
            "fixed" => {
                let fix: VerifiedFix = serde_json::from_value(
                    context
                        .context
                        .get("fix")
                        .cloned()
                        .ok_or_else(|| anyhow!("fixed action requires fix"))?,
                )?;
                learner
                    .mark_error_fixed(fingerprint, fix)
                    .map_err(|error| anyhow!(error.to_string()))?;
                Ok(json!({"status":"fixed","fingerprint":fingerprint}))
            }
            "verified" => {
                let version = context
                    .context
                    .get("version")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("verified action requires version"))?;
                learner
                    .mark_error_verified(fingerprint, version, now_secs())
                    .map_err(|error| anyhow!(error.to_string()))?;
                Ok(json!({"status":"verified","fingerprint":fingerprint,"version":version}))
            }
            _ => bail!("error_knowledge action must be get, reproduced, fixed, or verified"),
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
