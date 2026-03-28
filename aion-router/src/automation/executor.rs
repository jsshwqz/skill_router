use crate::automation::state::AutomationStep;
use serde_json::Value;
use std::collections::HashMap;

pub trait Executor {
    fn execute(&self, step: &AutomationStep, global_context: &HashMap<String, Value>) -> anyhow::Result<()>;
    fn rollback(&self, step: &AutomationStep, global_context: &HashMap<String, Value>) -> anyhow::Result<()>;
}
