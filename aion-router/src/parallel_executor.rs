use anyhow::{Result, anyhow};
use std::collections::HashMap;
use aion_types::types::{ExecutionContext, RouterPaths};
use aion_types::parallel::{ParallelInstruction, TaskGraph, ParallelResponse};
use aion_types::capability_registry::CapabilityRegistry;
use crate::executor::Executor;
use crate::loader::Loader;

pub struct ParallelExecutor;

impl ParallelExecutor {
    /// Execute a task graph in parallel using tokio tasks.
    ///
    /// Instructions are executed level by level: instructions whose dependencies
    /// are satisfied run concurrently within the same level via `tokio::task::JoinSet`.
    pub async fn execute_graph(
        graph: TaskGraph,
        paths: &RouterPaths,
        capability_registry: &CapabilityRegistry,
    ) -> Result<ParallelResponse> {
        let local_skills = Loader::load_local_skills(paths, capability_registry)?;
        let mut results: HashMap<String, serde_json::Value> = HashMap::new();
        let mut completed_ids: Vec<String> = Vec::new();

        // Level-based execution (DAG)
        while completed_ids.len() < graph.instructions.len() {
            let executable: Vec<ParallelInstruction> = graph.instructions.iter()
                .filter(|item| !completed_ids.contains(&item.id))
                .filter(|item| item.dependencies.iter().all(|dep| completed_ids.contains(dep)))
                .cloned()
                .collect();

            if executable.is_empty() && completed_ids.len() < graph.instructions.len() {
                return Err(anyhow!("Deadlock detected in TaskGraph or dependencies not met."));
            }

            // Execute current level in parallel using tokio JoinSet
            let mut join_set = tokio::task::JoinSet::new();

            for instr in executable {
                let skill = local_skills.iter()
                    .find(|s| s.supports_capability(&instr.capability))
                    .cloned();
                let paths_clone = paths.clone();

                join_set.spawn(async move {
                    let result_value = match skill {
                        Some(s) => {
                            let ctx = ExecutionContext::new(&instr.task, &instr.capability);
                            match Executor::execute(&s, &ctx, &paths_clone).await {
                                Ok(resp) => resp.result,
                                Err(e) => serde_json::json!({"error": e.to_string()}),
                            }
                        }
                        None => serde_json::json!({"error": format!("no skill found for capability: {}", instr.capability)}),
                    };
                    (instr.id, result_value)
                });
            }

            // Collect results from this level
            while let Some(result) = join_set.join_next().await {
                match result {
                    Ok((id, value)) => {
                        completed_ids.push(id.clone());
                        results.insert(id, value);
                    }
                    Err(e) => {
                        tracing::error!("Parallel task panicked: {}", e);
                    }
                }
            }
        }

        Ok(ParallelResponse { results })
    }
}
