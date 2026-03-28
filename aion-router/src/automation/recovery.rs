use crate::automation::error::AutomationError;
use crate::automation::state::SideEffectClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDecision {
    RetryStep,
    Replan,
    RollbackAndRetry,
    RollbackAndReplan,
    Abort(String),
}

pub struct RecoveryEngine;

impl RecoveryEngine {
    pub fn decide(
        error: &AutomationError,
        attempt_count: u32,
        max_attempts: u32,
        side_effect_class: &SideEffectClass,
        dirty_state: bool,
    ) -> RecoveryDecision {
        if attempt_count >= max_attempts {
            return RecoveryDecision::Abort("Max attempts reached".to_string());
        }

        // If the state is tainted by an irreversible action, we must abort to prevent cascading failures.
        if dirty_state && matches!(side_effect_class, SideEffectClass::Irreversible) {
            return RecoveryDecision::Abort("Irreversible side effect tainted the state".to_string());
        }

        match error {
            AutomationError::Transient(_) => {
                match side_effect_class {
                    SideEffectClass::LocalWriteReversible => {
                        if dirty_state { RecoveryDecision::RollbackAndRetry } else { RecoveryDecision::RetryStep }
                    }
                    SideEffectClass::LocalWriteBestEffort | SideEffectClass::PureRead => {
                        RecoveryDecision::RetryStep
                    }
                    SideEffectClass::ExternalSideEffect | SideEffectClass::HighRiskHumanConfirm | SideEffectClass::Irreversible => {
                        RecoveryDecision::Abort(format!("Transient error on sensitive action ({:?})", side_effect_class))
                    }
                }
            }
            AutomationError::Plan(_) | AutomationError::Verification(_) | AutomationError::Execution(_) => {
                match side_effect_class {
                    SideEffectClass::LocalWriteReversible => {
                        if dirty_state { RecoveryDecision::RollbackAndReplan } else { RecoveryDecision::Replan }
                    }
                    SideEffectClass::LocalWriteBestEffort | SideEffectClass::PureRead => {
                        RecoveryDecision::Replan
                    }
                    SideEffectClass::ExternalSideEffect | SideEffectClass::HighRiskHumanConfirm | SideEffectClass::Irreversible => {
                        RecoveryDecision::Abort(format!("Critical failure on action state ({:?})", side_effect_class))
                    }
                }
            }
            AutomationError::Environment(e) => RecoveryDecision::Abort(format!("Environment issue: {}", e)),
            AutomationError::Policy(e) => RecoveryDecision::Abort(format!("Policy violation: {}", e)),
            AutomationError::Unknown(_) => RecoveryDecision::Abort("Unknown error occurred".to_string()),
        }
    }
}
