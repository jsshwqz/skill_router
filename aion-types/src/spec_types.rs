//! Spec-Driven Development 数据结构
//!
//! 大型代码改造的五阶段流水线：分析→分解→规划→执行→总结

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 五阶段枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseKind {
    Analyze,
    Decompose,
    Plan,
    Execute,
    Learn,
}

impl std::fmt::Display for PhaseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Analyze => write!(f, "analyze"),
            Self::Decompose => write!(f, "decompose"),
            Self::Plan => write!(f, "plan"),
            Self::Execute => write!(f, "execute"),
            Self::Learn => write!(f, "learn"),
        }
    }
}

/// 阶段/任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for PhaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// 单个子任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub status: PhaseStatus,
    #[serde(default)]
    pub test_strategy: String,
    #[serde(default)]
    pub rollback_plan: String,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 风险评估条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecRisk {
    pub area: String,
    /// "low" | "medium" | "high" | "critical"
    pub severity: String,
    pub description: String,
    #[serde(default)]
    pub mitigation: String,
}

/// 经验教训条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecLesson {
    pub phase: PhaseKind,
    pub content: String,
    pub success: bool,
    #[serde(default)]
    pub timestamp: u64,
}

/// 单个阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecPhase {
    pub kind: PhaseKind,
    #[serde(default)]
    pub status: PhaseStatus,
    #[serde(default)]
    pub started_at: Option<u64>,
    #[serde(default)]
    pub completed_at: Option<u64>,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

/// 顶层规格项目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecProject {
    pub project_id: String,
    pub goal: String,
    #[serde(default)]
    pub workspace_path: String,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    pub phases: Vec<SpecPhase>,
    #[serde(default)]
    pub tasks: Vec<SpecTask>,
    #[serde(default)]
    pub risks: Vec<SpecRisk>,
    #[serde(default)]
    pub lessons: Vec<SpecLesson>,
}

impl SpecProject {
    /// 创建新项目，初始化五个阶段
    pub fn new(project_id: String, goal: String, workspace_path: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let phases = vec![
            SpecPhase { kind: PhaseKind::Analyze, status: PhaseStatus::Pending, started_at: None, completed_at: None, output: None, error: None },
            SpecPhase { kind: PhaseKind::Decompose, status: PhaseStatus::Pending, started_at: None, completed_at: None, output: None, error: None },
            SpecPhase { kind: PhaseKind::Plan, status: PhaseStatus::Pending, started_at: None, completed_at: None, output: None, error: None },
            SpecPhase { kind: PhaseKind::Execute, status: PhaseStatus::Pending, started_at: None, completed_at: None, output: None, error: None },
            SpecPhase { kind: PhaseKind::Learn, status: PhaseStatus::Pending, started_at: None, completed_at: None, output: None, error: None },
        ];

        Self {
            project_id,
            goal,
            workspace_path,
            created_at: now,
            updated_at: now,
            phases,
            tasks: Vec::new(),
            risks: Vec::new(),
            lessons: Vec::new(),
        }
    }

    /// 获取指定阶段的可变引用
    pub fn phase_mut(&mut self, kind: PhaseKind) -> Option<&mut SpecPhase> {
        self.phases.iter_mut().find(|p| p.kind == kind)
    }

    /// 获取指定阶段
    pub fn phase(&self, kind: PhaseKind) -> Option<&SpecPhase> {
        self.phases.iter().find(|p| p.kind == kind)
    }

    /// 更新时间戳
    pub fn touch(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// 找到下一个可执行的任务（所有依赖已完成）
    pub fn next_executable_task(&self) -> Option<&SpecTask> {
        self.tasks.iter().find(|t| {
            t.status == PhaseStatus::Pending
                && t.depends_on.iter().all(|dep| {
                    self.tasks
                        .iter()
                        .find(|d| d.id == *dep)
                        .map(|d| d.status == PhaseStatus::Completed)
                        .unwrap_or(true)
                })
        })
    }

    /// 检查所有任务是否完成
    pub fn all_tasks_done(&self) -> bool {
        !self.tasks.is_empty()
            && self.tasks.iter().all(|t| {
                t.status == PhaseStatus::Completed || t.status == PhaseStatus::Failed
            })
    }
}
