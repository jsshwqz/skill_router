use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::automation::error::AutomationError;
use crate::automation::verifier::VerificationReport;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutomationEvent {
    TaskStarted,
    StepStarted { step_id: String },
    StepExecuted { step_id: String },
    StepVerified { step_id: String, success: bool },
    SideEffectOccurred { step_id: String, class: SideEffectClass },
    RecoveryDecision { step_id: String, strategy: String },
    UserAcknowledgment { step_id: String, token: String },
    ErrorOccurred { step_id: Option<String>, error_message: String },
    TaskCompleted,
    TaskFailed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub timestamp_ms: u128,
    pub event: AutomationEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutomationStatus {
    Planning,
    Executing,
    Verifying,
    Recovering,
    Paused,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SideEffectClass {
    PureRead,
    LocalWriteReversible,
    LocalWriteBestEffort,
    ExternalSideEffect,
    HighRiskHumanConfirm,
    Irreversible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackContract {
    pub strategy: String, // e.g., "restore_backup", "delete_file"
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Executing,
    Verifying,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStep {
    pub id: String,
    pub title: String,
    pub status: StepStatus,
    pub capability: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub skill_id: Option<String>,
    pub inputs: Value,
    pub expected_outputs: Vec<String>,
    pub side_effect_class: SideEffectClass,
    pub rollback_contract: Option<RollbackContract>,
    pub verifier: Option<String>, // Verifier ID or Type
    pub attempt_count: u32,
    pub error: Option<AutomationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationState {
    pub session_id: String,
    pub status: AutomationStatus,
    pub plan_version: u32,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub replan_count: u32,
    pub max_replan_count: u32,
    pub dirty_state: bool, // Indicates if there are unverified side effects
    pub verification_reports: Vec<VerificationReport>,
    pub current_step_index: usize,
    pub steps: Vec<AutomationStep>,
    pub global_context: HashMap<String, Value>,
    pub error_history: Vec<AutomationError>,
    #[serde(default)]
    pub event_stream: Vec<EventEntry>,
}

impl AutomationState {
    pub fn new(session_id: &str, goal: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            status: AutomationStatus::Planning,
            plan_version: 1,
            attempt_count: 0,
            max_attempts: 10,
            replan_count: 0,
            max_replan_count: 5,
            dirty_state: false,
            verification_reports: Vec::new(),
            current_step_index: 0,
            steps: Vec::new(),
            global_context: [("goal".to_string(), Value::String(goal.to_string()))]
                .into_iter()
                .collect(),
            error_history: Vec::new(),
            event_stream: Vec::new(),
        }
    }

    pub fn save(&self, base_path: &std::path::Path) -> anyhow::Result<()> {
        let path = base_path.join(format!("{}.json", self.session_id));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(base_path: &std::path::Path, session_id: &str) -> anyhow::Result<Self> {
        let path = base_path.join(format!("{}.json", session_id));
        let content = std::fs::read_to_string(path)?;
        let state = serde_json::from_str(&content)?;
        Ok(state)
    }

    pub fn push_event(&mut self, event: AutomationEvent) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        
        self.event_stream.push(EventEntry {
            timestamp_ms,
            event,
        });
    }
}
