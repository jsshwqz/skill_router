//! 服务器事件总线
//!
//! 定义任务生命周期事件和基于 `tokio::sync::broadcast` 的事件分发。
//! WebSocket handler 订阅此总线，按 session_id 过滤推送给客户端。

use serde::Serialize;
use tokio::sync::broadcast;

/// 服务器推送事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// 任务开始执行
    TaskStarted {
        session_id: String,
        task: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        capability: Option<String>,
        timestamp: i64,
    },
    /// 任务执行完成
    TaskCompleted {
        session_id: String,
        skill: String,
        status: String,
        duration_ms: u64,
        timestamp: i64,
    },
    /// 任务执行失败
    TaskFailed {
        session_id: String,
        error: String,
        timestamp: i64,
    },
    /// Agent 间委派跳转
    DelegationHop {
        session_id: String,
        from: String,
        to: String,
        reason: String,
        timestamp: i64,
    },
}

impl ServerEvent {
    /// 返回事件关联的 session_id
    pub fn session_id(&self) -> &str {
        match self {
            Self::TaskStarted { session_id, .. } => session_id,
            Self::TaskCompleted { session_id, .. } => session_id,
            Self::TaskFailed { session_id, .. } => session_id,
            Self::DelegationHop { session_id, .. } => session_id,
        }
    }
}

/// 事件总线（基于 tokio broadcast channel）
pub struct EventBus {
    tx: broadcast::Sender<ServerEvent>,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// 发布事件
    pub fn publish(&self, event: ServerEvent) -> usize {
        self.tx.send(event).unwrap_or(0)
    }

    /// 订阅事件流
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.tx.subscribe()
    }
}
