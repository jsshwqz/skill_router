use anyhow::{anyhow, Result};
use crate::automation::state::{AutomationState, SideEffectClass};
use crate::registry::RegistryStore;
use aion_types::types::RouterPaths;

pub struct PlanValidator;

impl PlanValidator {
    pub fn validate(state: &AutomationState, paths: &RouterPaths) -> Result<()> {
        let registry = RegistryStore::load(paths)?;
        
        let mut seen_ids = std::collections::HashSet::new();
        let mut adj = std::collections::HashMap::new();

        // 1. First pass: Check ID uniqueness and build the graph
        for step in &state.steps {
            if !seen_ids.insert(step.id.clone()) {
                return Err(anyhow!("Duplicate step ID found: {}", step.id));
            }
            adj.insert(step.id.clone(), &step.dependencies);
        }

        // 2. Second pass: Validation per step
        for step in &state.steps {
            // 2.1 Check capability existence
            if !registry.skill_names().any(|name| name == step.capability) {
                return Err(anyhow!("Unknown capability '{}' required by step '{}'", step.capability, step.id));
            }

            // 2.2 Check dependencies exist and no self-dependency
            for dep in &step.dependencies {
                if dep == &step.id {
                    return Err(anyhow!("Step '{}' cannot depend on itself", step.id));
                }
                if !seen_ids.contains(dep) {
                    return Err(anyhow!("Step '{}' depends on unknown step '{}'", step.id, dep));
                }
            }

            // 3. Side Effect specific checks
            match step.side_effect_class {
                SideEffectClass::HighRiskHumanConfirm => {
                    // [Phase 1.9] Placeholder: In production, we'd check for a 'confirmed' metadata/token
                }
                SideEffectClass::LocalWriteReversible | SideEffectClass::ExternalSideEffect | SideEffectClass::Irreversible => {
                    if step.verifier.is_none() {
                        return Err(anyhow!("Step '{}' (class {:?}) has side effects but no verifier bound", step.id, step.side_effect_class));
                    }
                }
                _ => {}
            }
        }

        // 4. Cycle Detection (DFS)
        fn has_cycle(
            u: &String, 
            adj: &std::collections::HashMap<String, &Vec<String>>, 
            visited: &mut std::collections::HashSet<String>, 
            rec_stack: &mut std::collections::HashSet<String>
        ) -> bool {
            visited.insert(u.clone());
            rec_stack.insert(u.clone());

            if let Some(neighbors) = adj.get(u) {
                for v in *neighbors {
                    if !visited.contains(v) {
                        if has_cycle(v, adj, visited, rec_stack) {
                            return true;
                        }
                    } else if rec_stack.contains(v) {
                        return true;
                    }
                }
            }

            rec_stack.remove(u);
            false
        }

        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();
        for step_id in seen_ids {
            if !visited.contains(&step_id)
                && has_cycle(&step_id, &adj, &mut visited, &mut rec_stack) {
                    return Err(anyhow!("Circular dependency detected in the plan involving step '{}'", step_id));
                }
        }

        Ok(())
    }
}
