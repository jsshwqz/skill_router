#[cfg(test)]
mod tests {
    use crate::parallel_executor::ParallelExecutor;
    use aion_types::capability_registry::CapabilityRegistry;
    use aion_types::types::RouterPaths;
    use aion_types::parallel::{ParallelInstruction, TaskGraph};
    use std::env;
    use std::fs;

    #[test]
    fn test_parallel_5_tasks() {
        let tmp = env::temp_dir().join("aion_router_test_parallel");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();
        let reg = CapabilityRegistry::builtin();

        let instructions: Vec<ParallelInstruction> = (0..5)
            .map(|i| ParallelInstruction {
                id: format!("task_{}", i),
                task: format!("echo test {}", i),
                capability: "echo".to_string(),
                dependencies: vec![],
            })
            .collect();

        let graph = TaskGraph { instructions };

        let result = ParallelExecutor::execute_graph(graph, &paths, &reg);
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.results.len(), 5);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_concurrent_routing() {
        use crate::SkillRouter;
        use std::sync::Arc;
        use std::thread;

        let tmp = env::temp_dir().join("aion_router_test_concurrent");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();

        let router = Arc::new(SkillRouter::new(paths).unwrap());
        let mut handlers = vec![];

        for i in 0..10 {
            let r = Arc::clone(&router);
            handlers.push(thread::spawn(move || {
                r.route(&format!("echo hello {}", i))
            }));
        }

        for h in handlers {
            let res = h.join().unwrap();
            assert!(res.is_ok(), "Concurrent routing failed: {:?}", res.err());
        }

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_end_to_end_evolution() {
        use crate::SkillRouter;
        
        let tmp = env::temp_dir().join("aion_router_test_evolution");
        let _ = fs::remove_dir_all(&tmp);
        let paths = RouterPaths::for_workspace(&tmp);
        paths.ensure_base_dirs().unwrap();

        let router = SkillRouter::new(paths.clone()).unwrap();
        
        // Task that definitely shouldn't exist locally
        let task = "navigate to the Andromeda galaxy";
        let result = router.route(task);
        
        assert!(result.is_ok(), "Evolution routing failed: {:?}", result.err());
        let res = result.unwrap();
        
        // Verify it was a generated (synthesized) skill
        assert!(res.skill.metadata.name.contains("placeholder"));
        
        // Verify files were actually created
        let skill_dir = res.skill.root_dir.clone();
        assert!(skill_dir.exists());
        assert!(skill_dir.join("skill.json").exists());
        assert!(skill_dir.join("README.md").exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
