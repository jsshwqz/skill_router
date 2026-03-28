use anyhow::Result;
use aion_types::types::RouterPaths;
use aion_types::capability_registry::CapabilityRegistry;
use crate::parallel_executor::ParallelExecutor;
use aion_intel::parallel_planner::ParallelPlanner;
use aion_types::parallel::ParallelResponse;

pub struct MultiSkillCoordinator;

impl MultiSkillCoordinator {
    pub fn process_task_parallel(task: &str, paths: &RouterPaths, reg: &CapabilityRegistry) -> Result<ParallelResponse> {
        // 1. Task Splitting via AI
        let graph = ParallelPlanner::split_task(task, paths)?;
        
        // 2. Parallel Execution
        ParallelExecutor::execute_graph(graph, paths, reg)
    }
}
