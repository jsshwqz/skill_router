use anyhow::Result;
use crate::automation::state::{AutomationState, AutomationStep};
use crate::automation::error::AutomationError;

/// The Planner trait abstracts the AI LLM call or rule-based logic to 
/// generate an initial plan and patch a plan of action when a step fails.
pub trait Planner: Send + Sync {
    /// Generate an initial plan based on a goal and global context
    fn generate_initial_plan(
        &self,
        goal: &str,
        global_context: &std::collections::HashMap<String, serde_json::Value>
    ) -> Result<Vec<AutomationStep>>;

    /// Provide a patch step (or multiple steps) to recover from a failure
    /// Returns the initial failed step's replacement or new injected steps.
    fn generate_patch(
        &self, 
        state: &AutomationState, 
        failed_step: &AutomationStep, 
        error_context: &AutomationError
    ) -> Result<Vec<AutomationStep>>;
}
