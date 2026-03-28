use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AutomationError {
    Transient(String),
    Plan(String),
    Execution(String),
    Verification(String),
    Environment(String),
    Policy(String),
    Unknown(String),
}

impl std::fmt::Display for AutomationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient(s) => write!(f, "Transient error: {}", s),
            Self::Plan(s) => write!(f, "Plan error: {}", s),
            Self::Execution(s) => write!(f, "Execution error: {}", s),
            Self::Verification(s) => write!(f, "Verification error: {}", s),
            Self::Environment(s) => write!(f, "Environment error: {}", s),
            Self::Policy(s) => write!(f, "Policy error: {}", s),
            Self::Unknown(s) => write!(f, "Unknown error: {}", s),
        }
    }
}

impl std::error::Error for AutomationError {}
