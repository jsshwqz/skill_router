use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use aion_types::types::{ExecutionContext, RouterPaths};
use aion_types::parallel::{ParallelInstruction, TaskGraph, ParallelResponse};
use aion_types::capability_registry::CapabilityRegistry;
use crate::executor::Executor;
use crate::loader::Loader;

pub struct ParallelExecutor;

impl ParallelExecutor {
    pub fn execute_graph(
        graph: TaskGraph, 
        paths: &RouterPaths,
        capability_registry: &CapabilityRegistry
    ) -> Result<ParallelResponse> {
        let results = Arc::new(Mutex::new(HashMap::new()));
        let local_skills = Loader::load_local_skills(paths, capability_registry)?;

        // Simple parallel execution using a thread pool or rayon for each level of the DAG
        // For simplicity in this demo, we run non-dependent tasks in parallel
        // In a full implementation, we'd use a proper task scheduler like tokio
        
        let mut completed_ids = Vec::new();
        let _remaining_tasks = graph.instructions.clone();

        // Level-based execution (simplified DAG)
        while completed_ids.len() < graph.instructions.len() {
            let executable: Vec<ParallelInstruction> = graph.instructions.iter()
                .filter(|item| !completed_ids.contains(&item.id))
                .filter(|item| item.dependencies.iter().all(|dep| completed_ids.contains(dep)))
                .cloned()
                .collect();

            if executable.is_empty() && completed_ids.len() < graph.instructions.len() {
                return Err(anyhow!("Deadlock detected in TaskGraph or dependencies not met."));
            }

            let current_results = Arc::clone(&results);
            let paths_ref = paths.clone();
            let skills_ref = local_skills.clone();

            // Parallel execution of the current 'level'
            use rayon::prelude::*;
            executable.into_par_iter().for_each(|instr| {
                let skill = skills_ref.iter().find(|s| s.supports_capability(&instr.capability));
                let result_value = match skill {
                    Some(s) => {
                        let ctx = ExecutionContext::new(&instr.task, &instr.capability);
                        match Executor::execute(s, &ctx, &paths_ref) {
                            Ok(resp) => resp.result,
                            Err(e) => serde_json::json!({"error": e.to_string()}),
                        }
                    }
                    None => serde_json::json!({"error": format!("no skill found for capability: {}", instr.capability)}),
                };
                let mut lock = current_results.lock().unwrap();
                lock.insert(instr.id.clone(), result_value);
            });

            for item in &graph.instructions {
                let lock = results.lock().unwrap();
                if lock.contains_key(&item.id) && !completed_ids.contains(&item.id) {
                    completed_ids.push(item.id.clone());
                }
            }
        }

        let final_results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
        Ok(ParallelResponse { results: final_results })
    }
}
