use anyhow::{anyhow, Result};
use tracing::{info, warn, error};
use crate::automation::state::{AutomationState, AutomationStatus, StepStatus, SideEffectClass, AutomationEvent, EventEntry};
use crate::automation::error::AutomationError;
use crate::automation::plan_validator::PlanValidator;
use crate::automation::recovery::{RecoveryEngine, RecoveryDecision};
use crate::automation::verifier::Verifier;
use crate::automation::planner::Planner;
use aion_types::types::RouterPaths;

pub struct Orchestrator;

impl Orchestrator {
    pub fn run(
        mut state: AutomationState, 
        paths: &RouterPaths,
        planner: &dyn Planner,
        executor: &dyn crate::automation::executor::Executor,
        verifier_resolver: &dyn Fn(&str) -> Option<Box<dyn Verifier>>
    ) -> Result<AutomationState> {
        if !state.event_stream.iter().any(|e| matches!(e.event, AutomationEvent::TaskStarted)) {
            let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
            state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::TaskStarted });
        }

        loop {
            state.save(&paths.state_dir)?;

            match state.status {
                AutomationStatus::Planning => {
                    // Check logic for initial plan
                    if state.steps.is_empty() {
                        let goal = state.global_context.get("goal").cloned().unwrap_or_default();
                        let goal_str = goal.as_str().unwrap_or("undefined goal");
                        info!("Generating initial plan for goal: {}", goal_str);
                        
                        match planner.generate_initial_plan(goal_str, &state.global_context) {
                            Ok(initial_steps) => {
                                state.steps = initial_steps;
                                state.current_step_index = 0;
                            }
                            Err(e) => {
                                state.status = AutomationStatus::Failed;
                                return Err(anyhow!("Failed to generate initial plan: {}", e));
                            }
                        }
                    }

                    PlanValidator::validate(&state, paths).map_err(|e| {
                         state.status = AutomationStatus::Failed;
                         anyhow!("Plan validation failed: {}", e)
                    })?;
                    state.status = AutomationStatus::Executing;
                }
                AutomationStatus::Executing => {
                    if state.current_step_index >= state.steps.len() {
                        state.status = AutomationStatus::Completed;
                        continue;
                    }

                    let step = &mut state.steps[state.current_step_index];
                    step.status = StepStatus::Executing;
                    
                    let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                    state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::StepStarted { step_id: step.id.clone() } });
                    
                    // [Phase 1.95/1.96] High Risk Confirmation Gate (PRE-EXECUTION)
                    if step.side_effect_class == SideEffectClass::HighRiskHumanConfirm {
                        let confirmed = state.global_context.get("user_ack_token")
                            .and_then(|v| v.as_str())
                            .map(|t| t == "APPROVED_BY_USER") // In production, match session/version
                            .unwrap_or(false);
                        
                        if !confirmed {
                            let err = AutomationError::Policy("High-risk action requires user confirmation token (Blocked before execution)".to_string());
                            step.status = StepStatus::Failed;
                            step.error = Some(err.clone());
                            state.error_history.push(err.clone());
                            
                            let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                            state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::ErrorOccurred { step_id: Some(step.id.clone()), error_message: err.to_string() } });
                            
                            state.status = AutomationStatus::Failed;
                            return Err(anyhow!("Policy violation: {}", err));
                        } else {
                            let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                            state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::UserAcknowledgment { step_id: step.id.clone(), token: "APPROVED_BY_USER".to_string() } });
                        }
                    }

                    // Calling Executor
                    info!("Executing step [{}]: {}", step.id, step.title);
                    executor.execute(step, &state.global_context).map_err(|e| {
                        let err = AutomationError::Execution(e.to_string());
                        step.error = Some(err.clone());
                        state.error_history.push(err.clone()); // [Phase 1.9] Push to error history
                        
                        let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                        state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::ErrorOccurred { step_id: Some(step.id.clone()), error_message: e.to_string() } });
                        
                        err
                    })?;
                    
                    let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                    state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::StepExecuted { step_id: step.id.clone() } });

                    // [Phase 1.9] Broaden dirty_state coverage to all possible side effect classes
                    if !matches!(step.side_effect_class, SideEffectClass::PureRead) {
                        state.dirty_state = true;
                        
                        let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                        state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::SideEffectOccurred { step_id: step.id.clone(), class: step.side_effect_class.clone() } });
                    }

                    state.status = AutomationStatus::Verifying;
                }
                AutomationStatus::Verifying => {
                    let step = &mut state.steps[state.current_step_index];
                    step.status = StepStatus::Verifying;

                    info!("Verifying step [{}]: {}", step.id, step.title);
                    
                    let mut success = true;
                    if let Some(verifier_id) = &step.verifier {
                        if let Some(verifier) = verifier_resolver(verifier_id) {
                            match verifier.verify(&step.title, &step.inputs, &state.global_context.get("goal").cloned().unwrap_or_default()) {
                                Ok(report) => {
                                    state.verification_reports.push(report.clone());
                                    
                                    let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                                    state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::StepVerified { step_id: step.id.clone(), success: report.success } });
                                    
                                    if !report.success {
                                        success = false;
                                        warn!("Verification failed for step [{}]: {}", step.id, report.message);
                                        // [Phase 1.9] Track verification failures in error history
                                        state.error_history.push(AutomationError::Verification(report.message.clone()));
                                    } else {
                                        info!("Verification succeeded for step [{}]", step.id);
                                    }
                                }
                                Err(e) => {
                                    success = false;
                                    error!("Verifier '{}' execution failed for step [{}]: {}", verifier_id, step.id, e);
                                    let err = AutomationError::Verification(e.to_string());
                                    step.error = Some(err.clone());
                                    state.error_history.push(err); // [Phase 1.9]
                                    
                                    let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                                    state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::ErrorOccurred { step_id: Some(step.id.clone()), error_message: e.to_string() } });
                                }
                            }
                        } else {
                            warn!("Verifier '{}' not found for step [{}]", verifier_id, step.id);
                            // [Phase 1.96] Record missing verifier in history
                            let err = AutomationError::Policy(format!("Verifier '{}' not found for step '{}'", verifier_id, step.id));
                            state.error_history.push(err);
                            success = false;
                        }
                    } else {
                        // Phase 1 acceptance: high risk without verifier -> fail? 
                        // Currently handled in PlanValidator (fails planning).
                    }

                    if success {
                        state.steps[state.current_step_index].status = StepStatus::Completed;
                        state.current_step_index += 1;
                        state.status = AutomationStatus::Executing;
                    } else {
                        state.status = AutomationStatus::Recovering;
                    }
                }
                AutomationStatus::Recovering => {
                    state.attempt_count += 1;
                    
                    let failed_step = &state.steps[state.current_step_index];
                    let last_error = failed_step.error.clone().unwrap_or(AutomationError::Execution("Step failed verification".to_string()));
                    
                    let decision = RecoveryEngine::decide(
                        &last_error, 
                        state.attempt_count, 
                        state.max_attempts,
                        &failed_step.side_effect_class,
                        state.dirty_state
                    );

                    {
                        let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                        let strategy_str = match &decision {
                            RecoveryDecision::RetryStep => "RetryStep",
                            RecoveryDecision::Replan => "Replan",
                            RecoveryDecision::RollbackAndReplan => "RollbackAndReplan",
                            RecoveryDecision::Abort(_) => "Abort",
                            RecoveryDecision::RollbackAndRetry => "RollbackAndRetry",
                        };
                        state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::RecoveryDecision { step_id: failed_step.id.clone(), strategy: strategy_str.to_string() } });
                    }

                    match decision {
                        RecoveryDecision::RetryStep => {
                            state.error_history.push(AutomationError::Transient(format!("Transient error on step '{}', triggering simple retry", failed_step.id)));
                            state.status = AutomationStatus::Executing;
                        }
                        RecoveryDecision::Replan | RecoveryDecision::RollbackAndReplan => {
                            if decision == RecoveryDecision::RollbackAndReplan {
                                let failed_step = &state.steps[state.current_step_index];
                                if let Some(contract) = &failed_step.rollback_contract {
                                    info!("Executing rollback contract '{}' for step [{}]", contract.strategy, failed_step.id);
                                    
                                    // [Phase 1.95] Record decision before action
                                    state.error_history.push(AutomationError::Execution(format!("Replan required, triggering rollback ({}) for step '{}'", contract.strategy, failed_step.id)));

                                    executor.rollback(failed_step, &state.global_context).map_err(|e| {
                                        anyhow!("Rollback failed for step '{}': {}", failed_step.id, e)
                                    })?;
                                } else {
                                    warn!("Step '{}' requires rollback but no contract found — proceeding with replan anyway", failed_step.id);
                                    state.error_history.push(AutomationError::Execution(format!("Replan triggered WITHOUT rollback (missing contract) for step '{}'", failed_step.id)));
                                }
                                state.dirty_state = false;
                            } else {
                                // Record plain replan
                                state.error_history.push(AutomationError::Execution("Verification failed, triggering re-plan".to_string()));
                            }

                            state.replan_count += 1;
                            if state.replan_count > state.max_replan_count {
                                state.status = AutomationStatus::Failed;
                                return Err(anyhow!("Max replan count ({}) reached, aborting to prevent infinite loops.", state.max_replan_count));
                            }

                            state.plan_version += 1;
                            state.status = AutomationStatus::Planning;
                            info!("Re-planning required: plan_version={}, replan_count={}/{}", state.plan_version, state.replan_count, state.max_replan_count);

                            if state.current_step_index < state.steps.len() {
                                {
                                    let failed_step = &mut state.steps[state.current_step_index];
                                    failed_step.status = StepStatus::Failed;
                                }
                                
                                let failed_step_ref = &state.steps[state.current_step_index];
                                let last_error = failed_step_ref.error.clone().unwrap_or(AutomationError::Verification("Unknown verification failure".to_string()));
                                
                                match planner.generate_patch(&state, failed_step_ref, &last_error) {
                                    Ok(patch_steps) => {
                                        for fix_step in patch_steps.into_iter().rev() {
                                            info!("Injecting patch step [{}] into plan", fix_step.id);
                                            state.steps.insert(state.current_step_index + 1, fix_step);
                                        }
                                        // Move to the first injected patch step
                                        state.current_step_index += 1;
                                    }
                                    Err(e) => {
                                        state.status = AutomationStatus::Failed;
                                        return Err(anyhow!("Failed to generate patch plan: {}", e));
                                    }
                                }
                            }
                        }
                        RecoveryDecision::Abort(reason) => {
                            state.status = AutomationStatus::Failed;
                            return Err(anyhow!("Automation aborted: {}", reason));
                        }
                        RecoveryDecision::RollbackAndRetry => {
                            let failed_step = &state.steps[state.current_step_index];
                            if let Some(contract) = &failed_step.rollback_contract {
                                info!("Executing rollback+retry contract '{}' for step [{}]", contract.strategy, failed_step.id);
                                
                                // [Phase 1.95] Record decision
                                state.error_history.push(AutomationError::Execution(format!("Retry required with cleanup, triggering rollback ({}) for step '{}'", contract.strategy, failed_step.id)));

                                executor.rollback(failed_step, &state.global_context).map_err(|e| {
                                    anyhow!("Rollback failed for step '{}': {}", failed_step.id, e)
                                })?;
                            } else {
                                warn!("Step '{}' requires rollback but no contract found — retrying directly", failed_step.id);
                                state.error_history.push(AutomationError::Execution(format!("Retry triggered WITHOUT rollback (missing contract) for step '{}'", failed_step.id)));
                            }
                            state.dirty_state = false;
                            state.status = AutomationStatus::Executing;
                        }
                    }
                }
                AutomationStatus::Completed | AutomationStatus::Failed | AutomationStatus::Paused => {
                    let timestamp_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                    if state.status == AutomationStatus::Completed {
                        state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::TaskCompleted });
                    } else if state.status == AutomationStatus::Failed {
                        let reason = state.error_history.last().map(|e| e.to_string()).unwrap_or_else(|| "Unknown".to_string());
                        state.event_stream.push(EventEntry { timestamp_ms, event: AutomationEvent::TaskFailed { reason } });
                    }
                    break;
                }
            }
        }

        state.save(&paths.state_dir)?;
        Ok(state)
    }
}
