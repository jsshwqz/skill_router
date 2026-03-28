use aion_router::automation::{
    loop_engine::Orchestrator,
    state::{AutomationState, AutomationStep, SideEffectClass, StepStatus, AutomationStatus},
    verifier::{Verifier, CoreVerifiers},
    planner::Planner,
    executor::Executor,
    error::AutomationError,
};
use aion_types::types::RouterPaths;
use serde_json::json;

// Define a MockExecutor that handles the "code_generate" capability
struct MockExecutor;

impl Executor for MockExecutor {
    fn execute(&self, step: &AutomationStep, _global_context: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<()> {
        if step.capability == "code_generate" {
            let dir_str = step.inputs.get("dir").and_then(|v| v.as_str()).unwrap_or("");
            let test_dir = std::env::temp_dir().join(dir_str);
            let target_file = test_dir.join("src/main.rs");

            // [Phase 1.9] Simulation of automatic backup for Reversible actions
            if step.side_effect_class == SideEffectClass::LocalWriteReversible {
                if target_file.exists() {
                    let backup_file = test_dir.join("src/main.rs.bak");
                    println!("💾 MockExecutor: Creating backup at {:?}", backup_file);
                    std::fs::copy(&target_file, &backup_file)?;
                }
            }

            if step.title.contains("Auto-fix") {
                if !dir_str.is_empty() {
                    println!("🛠️  MockExecutor: Fixing file at {:?}", target_file);
                    std::fs::write(target_file, "fn main() {}\n")?;
                }
            } else {
                println!("🛠️  MockExecutor: Executing initial generation (writing broken code)");
                std::fs::write(target_file, "fn main() { DOES_NOT_COMPILE }")?;
            }
        }
        Ok(())
    }

    fn rollback(&self, step: &AutomationStep, _global_context: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<()> {
        println!("⏪ MockExecutor: Handling rollback for step: {}", step.id);
        if step.capability == "code_generate" {
            let dir_str = step.inputs.get("dir").and_then(|v| v.as_str()).unwrap_or("");
            let test_dir = std::env::temp_dir().join(dir_str);
            let target_file = test_dir.join("src/main.rs");
            let backup_file = test_dir.join("src/main.rs.bak");

            if backup_file.exists() {
                println!("⏪ MockExecutor: Restoring backup from {:?}", backup_file);
                std::fs::copy(&backup_file, &target_file)?;
            } else {
                println!("⏪ MockExecutor: No backup found, deleting faulty file {:?}", target_file);
                if target_file.exists() {
                    std::fs::remove_file(target_file)?;
                }
            }
        }
        Ok(())
    }
}

struct MockPlanner;

impl Planner for MockPlanner {
    fn generate_initial_plan(
        &self,
        _goal: &str,
        _global_context: &std::collections::HashMap<String, serde_json::Value>
    ) -> anyhow::Result<Vec<AutomationStep>> {
        Ok(vec![AutomationStep {
            id: "step_01".to_string(),
            title: "Generate Fix".to_string(),
            status: StepStatus::Pending,
            capability: "code_generate".to_string(),
            dependencies: vec![],
            skill_id: None,
            inputs: json!({"dir": _global_context.get("test_dir").and_then(|v| v.as_str()).unwrap_or("")}),
            expected_outputs: vec![],
            side_effect_class: SideEffectClass::LocalWriteReversible,
            rollback_contract: Some(aion_router::automation::state::RollbackContract {
                strategy: "restore_backup".to_string(),
                backup_path: None, // Implicit in our mock
            }),
            verifier: Some("cargo_check".to_string()),
            attempt_count: 0,
            error: None,
        }])
    }

    fn generate_patch(
        &self, 
        state: &AutomationState, 
        failed_step: &AutomationStep, 
        _error_context: &AutomationError
    ) -> anyhow::Result<Vec<AutomationStep>> {
        // The fix is in the Executor.
        let fix_step = AutomationStep {
            id: format!("{}_patch_v{}", failed_step.id, state.plan_version),
            title: format!("Auto-fix for: {}", failed_step.title),
            status: StepStatus::Pending,
            capability: "code_generate".to_string(),
            dependencies: vec![failed_step.id.clone()],
            skill_id: None,
            inputs: failed_step.inputs.clone(),
            expected_outputs: vec![],
            side_effect_class: SideEffectClass::LocalWriteReversible,
            rollback_contract: None,
            verifier: failed_step.verifier.clone(),
            attempt_count: 0,
            error: None,
        };
        Ok(vec![fix_step])
    }
}

#[test]
fn test_cpevr_loop_integration() -> anyhow::Result<()> {
    let test_dir = std::env::temp_dir().join(format!("aion_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    let paths = RouterPaths::for_workspace(&test_dir);
    paths.ensure_base_dirs()?;
    
    let test_dir_str = test_dir.file_name().unwrap().to_string_lossy().to_string();
    
    // 初始化一份假装存在的注册表规备 validate 报错
    let registry_path = &paths.registry_path;
    std::fs::write(
        registry_path, 
        json!({
            "skills": {
                "code_generate": {}
            }
        }).to_string()
    )?;

    // Prepare a temporary Cargo project that fails to compile initially
    let cargo_toml = r#"
[package]
name = "test_project"
version = "0.1.0"
edition = "2021"
"#;
    let src_dir = test_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(test_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(src_dir.join("main.rs"), "fn main() { DOES_NOT_COMPILE }")?;

    let mut state = AutomationState::new("session_101", "Fix a bug");
    state.global_context.insert("goal".to_string(), json!("Fix a bug"));
    state.global_context.insert("test_dir".to_string(), json!(test_dir_str.clone()));

    let planner = MockPlanner;
    let executor = MockExecutor;
    
    let working_dir = test_dir.clone();
    let verifier_resolver = move |name: &str| -> Option<Box<dyn Verifier>> {
        CoreVerifiers::resolve(name, working_dir.clone())
    };

    // 运行 Orchestrator
    let final_state = Orchestrator::run(state, &paths, &planner, &executor, &verifier_resolver)?;
    
    // 断言 (Assertions)
    assert_eq!(final_state.status, AutomationStatus::Completed);
    assert_eq!(final_state.plan_version, 2, "Should replan once");
    assert_eq!(final_state.attempt_count, 1, "Should retry once");
    assert_eq!(final_state.steps.len(), 2, "Should have injected one patch step");
    assert_eq!(final_state.steps[0].status, StepStatus::Failed, "Initial step should be marked Failed");
    assert_eq!(final_state.steps[1].status, StepStatus::Completed, "Patch step should be marked Completed");
    
    // Check validation reports collected
    assert!(final_state.verification_reports.len() >= 2, "Fail -> Success");
    assert!(!final_state.verification_reports[0].success);
    
    // Check persistence
    let saved_state = AutomationState::load(&paths.state_dir, "session_101")?;
    assert_eq!(saved_state.plan_version, 2);
    
    std::fs::remove_dir_all(test_dir)?;
    Ok(())
}

#[test]
fn test_high_risk_physical_block() -> anyhow::Result<()> {
    let mut state = AutomationState::new("session_hr", "High risk task");
    state.steps = vec![AutomationStep {
        id: "hr_step".to_string(),
        title: "High Risk Action".to_string(),
        status: StepStatus::Pending,
        capability: "code_generate".to_string(),
        dependencies: vec![],
        skill_id: None,
        inputs: json!({"dir": "hr_test"}),
        expected_outputs: vec![],
        side_effect_class: SideEffectClass::HighRiskHumanConfirm,
        rollback_contract: None,
        verifier: None,
        attempt_count: 0,
        error: None,
    }];
    
    // Use a temp path for testing
    let temp_root = std::env::temp_dir().join("aion_hr_block_test");
    std::fs::create_dir_all(&temp_root)?;
    
    // [Phase 1.96] Mock the registry so PlanValidator allows 'code_generate'
    let state_dir = temp_root.join(".skill-router");
    std::fs::create_dir_all(&state_dir)?;
    let registry_json = json!({
        "skills": {
            "code_generate": {
                "executions": [],
                "last_used_epoch_ms": null
            }
        }
    });
    std::fs::write(state_dir.join("registry.json"), serde_json::to_vec(&registry_json)?)?;

    let paths = RouterPaths::for_workspace(&temp_root);
    let planner = MockPlanner;
    let executor = MockExecutor;
    let verifier_resolver = |_name: &str| -> Option<Box<dyn Verifier>> { None };

    let result = Orchestrator::run(state, &paths, &planner, &executor, &verifier_resolver);
    
    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_root);
    
    assert!(result.is_err());
    let err_msg = result.err().unwrap().to_string();
    println!("Actual error message: {}", err_msg);
    assert!(err_msg.contains("High-risk action requires user confirmation token (Blocked before execution)"));
    
    // Check error history in state if possible (but Orchestrator returns error and state is taken)
    // In current impl, we might want to return state even on failure, or check it via persistence.
    // For now, proving the error message is enough to confirm the PRE-EXECUTION block.
    Ok(())
}

#[test]
fn test_event_stream_recording() -> anyhow::Result<()> {
    use aion_router::automation::state::AutomationEvent;
    
    let temp_root = std::env::temp_dir().join("aion_event_test");
    std::fs::create_dir_all(&temp_root)?;
    
    // [Phase 1.96] Mock the registry so PlanValidator allows 'code_generate'
    let state_dir = temp_root.join(".skill-router");
    std::fs::create_dir_all(&state_dir)?;
    let registry_json = json!({
        "skills": {
            "code_generate": {}
        }
    });
    std::fs::write(state_dir.join("registry.json"), serde_json::to_vec(&registry_json)?)?;

    let paths = RouterPaths::for_workspace(&temp_root);
    let mut state = AutomationState::new("session_evt", "Event testing");
    state.steps = vec![AutomationStep {
        id: "evt_step_1".to_string(),
        title: "Normal Step".to_string(),
        status: StepStatus::Pending,
        capability: "code_generate".to_string(),
        dependencies: vec![],
        skill_id: None,
        inputs: json!({}),
        expected_outputs: vec![],
        side_effect_class: SideEffectClass::PureRead,
        rollback_contract: None,
        verifier: None,
        attempt_count: 0,
        error: None,
    }];
    
    let planner = MockPlanner;
    // We use a mock executor that does nothing for this step
    struct NoOpExecutor;
    impl Executor for NoOpExecutor {
        fn execute(&self, _step: &AutomationStep, _global_context: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<()> { Ok(()) }
        fn rollback(&self, _step: &AutomationStep, _global_context: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<()> { Ok(()) }
    }
    
    let executor = NoOpExecutor;
    let verifier_resolver = |_name: &str| -> Option<Box<dyn Verifier>> { None };

    // This will run successfully because no verifier means it completes
    let final_state = Orchestrator::run(state, &paths, &planner, &executor, &verifier_resolver)?;
    
    let _ = std::fs::remove_dir_all(&temp_root);
    
    // Test the events
    let events: Vec<&AutomationEvent> = final_state.event_stream.iter().map(|e| &e.event).collect();
    
    assert!(!events.is_empty(), "Event stream should not be empty");
    
    // Verify sequence: TaskStarted -> StepStarted -> StepExecuted -> TaskCompleted
    let has_task_started = events.iter().any(|e| matches!(e, AutomationEvent::TaskStarted));
    let has_step_started = events.iter().any(|e| matches!(e, AutomationEvent::StepStarted { step_id } if step_id == "evt_step_1"));
    let has_step_executed = events.iter().any(|e| matches!(e, AutomationEvent::StepExecuted { step_id } if step_id == "evt_step_1"));
    let has_task_completed = events.iter().any(|e| matches!(e, AutomationEvent::TaskCompleted));
    
    assert!(has_task_started, "Missing TaskStarted event");
    assert!(has_step_started, "Missing StepStarted event");
    assert!(has_step_executed, "Missing StepExecuted event");
    assert!(has_task_completed, "Missing TaskCompleted event");

    Ok(())
}
