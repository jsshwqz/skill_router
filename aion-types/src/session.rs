//! 有状态会话管理
//!
//! 支持 undo/redo 的交互式会话，用于 REPL 和迭代式 Agent 工作流。

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::ExecutionResponse;

/// 撤销动作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Default)]
pub enum UndoAction {
    /// 从快照恢复文件
    RestoreFile {
        path: PathBuf,
        #[serde(with = "base64_bytes")]
        snapshot: Vec<u8>,
    },
    /// 执行补偿命令
    CompensatingCommand {
        command: String,
        args: Vec<String>,
    },
    /// 无法撤销（只读操作）
    #[default]
    None,
}

/// base64 序列化辅助
mod base64_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 简单的十六进制编码（避免引入 base64 crate）
        let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        serializer.serialize_str(&hex)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        (0..hex.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&hex[i..i + 2], 16)
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

/// 单步操作记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStep {
    /// 步骤 ID
    pub step_id: String,
    /// 时间戳
    pub timestamp: u64,
    /// 执行的能力
    pub capability: String,
    /// 原始任务
    pub task: String,
    /// 输入上下文
    #[serde(default)]
    pub context: Value,
    /// 执行结果
    pub result: ExecutionResponse,
    /// 撤销动作
    #[serde(default = "default_undo")]
    pub undo_action: UndoAction,
}

fn default_undo() -> UndoAction {
    UndoAction::None
}


/// 有状态会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// 会话 ID
    pub id: String,
    /// 创建时间
    pub created_at: u64,
    /// 操作历史
    pub steps: Vec<SessionStep>,
    /// 当前游标位置（指向最后已执行步骤的下一个位置）
    pub cursor: usize,
}

impl Session {
    /// 创建新会话
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            steps: Vec::new(),
            cursor: 0,
        }
    }

    /// 记录一步操作
    pub fn record_step(&mut self, step: SessionStep) {
        // 如果游标不在末尾，清除游标之后的步骤（redo 历史丢弃）
        self.steps.truncate(self.cursor);
        self.steps.push(step);
        self.cursor = self.steps.len();
    }

    /// 是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// 是否可以重做
    pub fn can_redo(&self) -> bool {
        self.cursor < self.steps.len()
    }

    /// 获取待撤销的步骤（不移动游标）
    pub fn peek_undo(&self) -> Option<&SessionStep> {
        if self.cursor > 0 {
            Some(&self.steps[self.cursor - 1])
        } else {
            None
        }
    }

    /// 执行撤销（移动游标）
    pub fn undo(&mut self) -> Option<&SessionStep> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(&self.steps[self.cursor])
        } else {
            None
        }
    }

    /// 执行重做（移动游标）
    pub fn redo(&mut self) -> Option<&SessionStep> {
        if self.cursor < self.steps.len() {
            let step = &self.steps[self.cursor];
            self.cursor += 1;
            Some(step)
        } else {
            None
        }
    }

    /// 获取命令历史摘要
    pub fn history_summary(&self) -> Vec<String> {
        self.steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let marker = if i < self.cursor { "+" } else { "-" };
                format!("[{}] {} {}: {}", marker, step.step_id, step.capability, step.task)
            })
            .collect()
    }

    /// 步骤数量
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_step(id: &str, capability: &str, task: &str) -> SessionStep {
        SessionStep {
            step_id: id.to_string(),
            timestamp: 0,
            capability: capability.to_string(),
            task: task.to_string(),
            context: Value::Null,
            result: ExecutionResponse {
                status: "ok".to_string(),
                result: Value::Null,
                artifacts: Value::Null,
                error: None,
            },
            undo_action: UndoAction::None,
        }
    }

    #[test]
    fn test_session_undo_redo() {
        let mut session = Session::new();

        session.record_step(make_step("1", "echo", "hello"));
        session.record_step(make_step("2", "echo", "world"));
        assert_eq!(session.cursor, 2);
        assert!(session.can_undo());
        assert!(!session.can_redo());

        // undo
        let step = session.undo().unwrap();
        assert_eq!(step.step_id, "2");
        assert_eq!(session.cursor, 1);
        assert!(session.can_redo());

        // redo
        let step = session.redo().unwrap();
        assert_eq!(step.step_id, "2");
        assert_eq!(session.cursor, 2);
        assert!(!session.can_redo());
    }

    #[test]
    fn test_session_record_clears_redo() {
        let mut session = Session::new();
        session.record_step(make_step("1", "a", "t1"));
        session.record_step(make_step("2", "b", "t2"));
        session.record_step(make_step("3", "c", "t3"));

        // undo twice
        session.undo();
        session.undo();
        assert_eq!(session.cursor, 1);

        // record new step — should clear steps 2 and 3
        session.record_step(make_step("4", "d", "t4"));
        assert_eq!(session.len(), 2);
        assert_eq!(session.steps[1].step_id, "4");
    }

    #[test]
    fn test_session_history_summary() {
        let mut session = Session::new();
        session.record_step(make_step("1", "echo", "hello"));
        session.record_step(make_step("2", "echo", "world"));
        session.undo();

        let summary = session.history_summary();
        assert_eq!(summary.len(), 2);
        assert!(summary[0].starts_with("[+]"));
        assert!(summary[1].starts_with("[-]"));
    }
}
