//! 多 Agent 协作的核心消息协议与角色定义
//!
//! # 设计原则
//! - 所有新字段使用 `#[serde(default)]`，确保向后兼容（旧客户端无需修改）
//! - `AgentMessage` 是 Agent 间通信的统一信封
//! - `DelegationHop` 追踪任务委派路径，防止循环委派

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Agent 角色 ───────────────────────────────────────────────────────────────

/// Agent 在协作网格中承担的角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// 总协调者：接收用户目标，分解任务，分配给其他 Agent
    #[default]
    Orchestrator,
    /// 规划者：专门负责将目标分解为执行步骤
    Planner,
    /// 通用执行者：执行具体的能力调用
    Executor,
    /// 专家执行者：专注于特定能力子集
    Specialist {
        /// 该 Specialist 负责的能力列表
        capabilities: Vec<String>,
    },
    /// 审查者：执行安全审查和结果验证
    Reviewer,
    /// 记忆管理者：专门维护 MemoryStore，处理 remember/recall/distill
    MemoryKeeper,
}

/// 对远程 Agent 的引用（地址 + 角色）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRef {
    /// Agent 唯一 ID（UUID v4）
    pub id: String,
    /// Agent 承担的角色
    pub role: AgentRole,
    /// 网络端点（None 表示进程内同节点）
    /// 格式：`http://host:port` 或 NATS subject prefix
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Agent 支持的能力列表（用于路由决策）
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl AgentRef {
    /// 创建一个本地（进程内）Agent 引用
    pub fn local(id: &str, role: AgentRole) -> Self {
        Self {
            id: id.to_string(),
            role,
            endpoint: None,
            capabilities: Vec::new(),
        }
    }

    /// 检查该 Agent 是否支持指定能力
    pub fn supports_capability(&self, cap: &str) -> bool {
        if self.capabilities.is_empty() {
            return true; // 未声明能力限制时接受所有能力
        }
        self.capabilities.iter().any(|c| c == cap)
    }
}

// ── 委派跳转记录 ─────────────────────────────────────────────────────────────

/// 记录一次 Agent 间的任务委派跳转
///
/// 用于构成 `delegation_chain`，追踪任务在 Agent 间的流转路径，
/// 防止循环委派，并用于分布式追踪（对应 OpenTelemetry span attribute）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationHop {
    /// 委派方 Agent ID
    pub from_agent: String,
    /// 受托方 Agent ID
    pub to_agent: String,
    /// 委派时间戳（UNIX 毫秒）
    pub timestamp_ms: u64,
    /// 委派原因
    /// 例如："capability_mismatch"、"load_balancing"、"specialist_required"
    pub reason: String,
}

impl DelegationHop {
    /// 创建一个新的委派跳转记录
    pub fn new(from: &str, to: &str, reason: &str) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            timestamp_ms,
            reason: reason.to_string(),
        }
    }
}

// ── Agent 消息类型 ────────────────────────────────────────────────────────────

/// Agent 间通信的消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AgentMessageType {
    /// 任务分配：Orchestrator → Executor/Specialist
    TaskAssignment {
        /// 任务 ID（对应 ParallelInstruction.id 或 AutomationStep.id）
        task_id: String,
        /// 任务描述
        task: String,
        /// 目标能力
        capability: String,
    },
    /// 任务执行结果：Executor/Specialist → Orchestrator
    TaskResult {
        /// 对应的任务 ID
        task_id: String,
        /// 是否成功
        success: bool,
        /// 执行结果（JSON）
        result: Value,
        /// 失败时的错误信息
        #[serde(default)]
        error: Option<String>,
    },
    /// 取消进行中的任务
    TaskCancel {
        task_id: String,
        reason: String,
    },
    /// 状态更新（进度通知）
    StatusUpdate {
        task_id: String,
        progress: f32,   // 0.0 - 1.0
        message: String,
    },
    /// 心跳（用于节点存活检测）
    HeartBeat {
        /// 当前节点负载（0.0 = 空闲，1.0 = 满负载）
        load: f32,
        /// 当前可用能力列表
        available_capabilities: Vec<String>,
    },
    /// 能力公告（新节点上线时广播）
    CapabilityAnnouncement {
        capabilities: Vec<String>,
        role: AgentRole,
    },
    /// 错误上报
    ErrorReport {
        task_id: Option<String>,
        error: String,
        recoverable: bool,
    },
}

// ── 统一消息信封 ──────────────────────────────────────────────────────────────

/// Agent 间通信的统一消息信封
///
/// 无论是进程内（tokio broadcast channel）还是跨网络（NATS subject），
/// 都使用此结构传递消息，保证协议一致性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// 消息唯一 ID（UUID v4）
    pub message_id: String,
    /// 发送方 Agent ID
    pub from_agent: String,
    /// 接收方 Agent ID（空字符串 = 广播给所有人）
    pub to_agent: String,
    /// 关联的 session ID（用于跨 Agent 会话追踪）
    #[serde(default)]
    pub session_id: String,
    /// 消息体
    pub message_type: AgentMessageType,
    /// 发送时间戳（UNIX 毫秒）
    pub timestamp_ms: u64,
    /// 关联消息 ID（用于 request-reply 模式，对应原始请求的 message_id）
    #[serde(default)]
    pub correlation_id: Option<String>,
}

impl AgentMessage {
    /// 创建新消息
    pub fn new(from: &str, to: &str, msg_type: AgentMessageType) -> Self {
        let message_id = uuid::Uuid::new_v4().to_string();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            message_id,
            from_agent: from.to_string(),
            to_agent: to.to_string(),
            session_id: String::new(),
            message_type: msg_type,
            timestamp_ms,
            correlation_id: None,
        }
    }

    /// 广播消息（发送给所有 Agent）
    pub fn broadcast(from: &str, msg_type: AgentMessageType) -> Self {
        Self::new(from, "", msg_type)
    }

    /// 设置 session ID
    pub fn with_session(mut self, session_id: &str) -> Self {
        self.session_id = session_id.to_string();
        self
    }

    /// 设置关联 ID（reply 场景）
    pub fn with_correlation(mut self, correlation_id: &str) -> Self {
        self.correlation_id = Some(correlation_id.to_string());
        self
    }

    /// 检查是否是广播消息
    pub fn is_broadcast(&self) -> bool {
        self.to_agent.is_empty()
    }

    /// 检查此消息是否发给指定 Agent
    pub fn is_for(&self, agent_id: &str) -> bool {
        self.is_broadcast() || self.to_agent == agent_id
    }
}

// ── Agent 状态 ────────────────────────────────────────────────────────────────

/// Agent 节点的当前运行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// 在线，等待任务
    #[default]
    Idle,
    /// 正在执行任务
    Busy,
    /// 已离线或不可达
    Offline,
    /// 暂停（管理员手动暂停）
    Paused,
}

/// Agent 节点的完整信息（用于服务发现和负载均衡）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent 唯一 ID
    pub id: String,
    /// 可读名称
    pub name: String,
    /// 角色
    pub role: AgentRole,
    /// 支持的能力列表
    pub capabilities: Vec<String>,
    /// 当前状态
    pub status: AgentStatus,
    /// 当前负载（0.0 = 空闲，1.0 = 满负载）
    pub load: f32,
    /// 网络地址（"local" 表示进程内，其他为 URL）
    pub location: String,
    /// 最后一次心跳时间戳（UNIX 毫秒）
    pub last_heartbeat_ms: u64,
}

impl AgentInfo {
    /// 创建一个本地进程内 Agent 信息
    pub fn local(id: &str, name: &str, role: AgentRole, capabilities: Vec<String>) -> Self {
        let last_heartbeat_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            id: id.to_string(),
            name: name.to_string(),
            role,
            capabilities,
            status: AgentStatus::Idle,
            load: 0.0,
            location: "local".to_string(),
            last_heartbeat_ms,
        }
    }
}
