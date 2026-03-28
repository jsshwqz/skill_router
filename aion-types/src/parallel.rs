use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── 执行模式 ─────────────────────────────────────────────────────────────────

/// 任务的并发执行模式
///
/// 控制同一任务是由单个 Agent 执行，还是多个 Agent 竞争/协作执行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// 默认：分配给指定（或最优）Agent 单独执行
    #[default]
    Single,
    /// 竞争执行：同时分配给多个 Agent，取第一个成功的结果
    /// 适用于对延迟敏感的任务
    Competitive,
    /// 法定人数执行：n 个 Agent 执行，取多数票结果
    /// 适用于对准确性要求高的任务（如代码审查、安全验证）
    Quorum(u8),
}

// ── 并行任务指令 ──────────────────────────────────────────────────────────────

/// 并行任务图中的单个指令节点
///
/// `dependencies` 字段定义 DAG 边，只有依赖项全部完成后才会执行当前指令。
/// `assignee_agent_id` 为 None 时，由调度器自动选择最优 Agent。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelInstruction {
    /// 指令唯一 ID（在 TaskGraph 内唯一）
    pub id: String,
    /// 自然语言任务描述
    pub task: String,
    /// 目标能力（如 "text_summarize"）
    pub capability: String,
    /// 依赖的指令 ID 列表（DAG 边）
    pub dependencies: Vec<String>,

    // ── 多 Agent 扩展字段（serde(default) 保证向后兼容）─────────────────────

    /// 指定执行此指令的 Agent ID
    /// None = 调度器自动选择（根据能力和负载）
    #[serde(default)]
    pub assignee_agent_id: Option<String>,

    /// 执行超时时间（秒），0 表示使用系统默认值（30s）
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// 执行失败时的最大重试次数
    #[serde(default)]
    pub max_retries: u32,

    /// 执行模式（默认 Single）
    #[serde(default)]
    pub execution_mode: ExecutionMode,
}

fn default_timeout_secs() -> u64 { 30 }

impl ParallelInstruction {
    /// 创建一个最简化的指令（无 Agent 分配，无依赖）
    pub fn simple(id: &str, task: &str, capability: &str) -> Self {
        Self {
            id: id.to_string(),
            task: task.to_string(),
            capability: capability.to_string(),
            dependencies: Vec::new(),
            assignee_agent_id: None,
            timeout_secs: 30,
            max_retries: 0,
            execution_mode: ExecutionMode::Single,
        }
    }

    /// 检查当前指令是否准备好执行（所有依赖已在 completed_ids 中）
    pub fn is_ready(&self, completed_ids: &std::collections::HashSet<String>) -> bool {
        self.dependencies.iter().all(|dep| completed_ids.contains(dep))
    }
}

// ── 任务图 ───────────────────────────────────────────────────────────────────

/// 多步骤并行任务的有向无环图（DAG）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub instructions: Vec<ParallelInstruction>,
}

impl TaskGraph {
    /// 获取当前可执行的指令（依赖已满足）
    pub fn ready_instructions<'a>(
        &'a self,
        completed_ids: &std::collections::HashSet<String>,
    ) -> Vec<&'a ParallelInstruction> {
        self.instructions
            .iter()
            .filter(|instr| !completed_ids.contains(&instr.id) && instr.is_ready(completed_ids))
            .collect()
    }
}

// ── 并行执行响应 ──────────────────────────────────────────────────────────────

/// 并行执行的汇总结果
pub struct ParallelResponse {
    /// 各指令 ID 对应的执行结果
    pub results: HashMap<String, serde_json::Value>,
}
