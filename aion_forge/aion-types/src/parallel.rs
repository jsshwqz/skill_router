use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelInstruction {
    pub id: String,
    pub task: String,
    pub capability: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub instructions: Vec<ParallelInstruction>,
}

pub struct ParallelResponse {
    pub results: HashMap<String, serde_json::Value>,
}
