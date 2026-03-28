//! Agent 运行时（进程内多 Agent MVP）
//!
//! 每个 `AgentRuntime` 在独立的 `tokio::task` 中运行，
//! 通过 `MessageBus` 接收任务并执行，实现进程内多 Agent 协作。
//!
//! # 架构
//! ```text
//! [Orchestrator]
//!      |  TaskAssignment → MessageBus
//!      ↓
//! [AgentRuntime A]  ─── SkillRouter (capabilities: text_*)
//! [AgentRuntime B]  ─── SkillRouter (capabilities: code_*)
//! [AgentRuntime C]  ─── SkillRouter (capabilities: web_*)
//!      |  TaskResult → MessageBus
//!      ↓
//! [Orchestrator] 汇总结果
//! ```

use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use aion_types::agent_message::{
    AgentInfo, AgentMessage, AgentMessageType, AgentRole,
};
use aion_types::types::RouterPaths;

use crate::message_bus::MessageBus;
use crate::SkillRouter;

/// 进程内 Agent 运行时
///
/// 持有一个 `SkillRouter` 实例，专注于该 Agent 负责的能力子集。
/// 通过 `MessageBus` 接收 `TaskAssignment`，执行后发布 `TaskResult`。
pub struct AgentRuntime {
    /// Agent 唯一 ID
    pub id: String,
    /// Agent 角色
    pub role: AgentRole,
    /// 消息总线（共享引用）
    bus: Arc<MessageBus>,
    /// 该 Agent 负责的能力列表（为空时接受所有能力）
    capabilities: Vec<String>,
    /// SkillRouter（每个 Agent 持有独立实例，避免 Mutex 竞争）
    router: Arc<SkillRouter>,
}

impl AgentRuntime {
    /// 创建新的 AgentRuntime
    pub fn new(
        id: &str,
        role: AgentRole,
        capabilities: Vec<String>,
        paths: RouterPaths,
        bus: Arc<MessageBus>,
    ) -> anyhow::Result<Self> {
        let router = Arc::new(SkillRouter::new(paths)?);
        Ok(Self {
            id: id.to_string(),
            role,
            bus,
            capabilities,
            router,
        })
    }

    /// 获取该 Agent 的信息摘要
    pub fn info(&self) -> AgentInfo {
        AgentInfo::local(&self.id, &self.id, self.role.clone(), self.capabilities.clone())
    }

    /// 检查该 Agent 是否处理指定能力
    pub fn handles_capability(&self, cap: &str) -> bool {
        if self.capabilities.is_empty() {
            return true;
        }
        self.capabilities.iter().any(|c| c == cap)
    }

    /// 启动 Agent，在后台 tokio task 中监听并处理消息
    ///
    /// 返回 `JoinHandle`，可用于等待 Agent 退出
    pub fn spawn(self) -> JoinHandle<()> {
        let agent_id = self.id.clone();
        let bus = Arc::clone(&self.bus);
        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            info!("Agent [{}] ({:?}) started", agent_id, self.role);

            // 广播上线公告
            let announcement = AgentMessage::broadcast(
                &agent_id,
                AgentMessageType::CapabilityAnnouncement {
                    capabilities: self.capabilities.clone(),
                    role: self.role.clone(),
                },
            );
            bus.publish(announcement);

            // 主消息循环
            loop {
                match rx.recv().await {
                    Ok(msg) => {
                        // 过滤：只处理发给自己（或广播）的消息
                        if !msg.is_for(&agent_id) {
                            continue;
                        }
                        self.handle_message(msg, &bus).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "Agent [{}] lagged, skipped {} messages",
                            agent_id, skipped
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Agent [{}] message bus closed, shutting down", agent_id);
                        break;
                    }
                }
            }

            info!("Agent [{}] stopped", agent_id);
        })
    }

    /// 处理单条消息
    async fn handle_message(&self, msg: AgentMessage, bus: &MessageBus) {
        match &msg.message_type {
            AgentMessageType::TaskAssignment { task_id, task, capability } => {
                // 检查是否处理该能力
                if !self.handles_capability(capability) {
                    warn!(
                        "Agent [{}] received task [{}] for unhandled capability '{}', ignoring",
                        self.id, task_id, capability
                    );
                    return;
                }

                info!(
                    "Agent [{}] executing task [{}]: capability='{}', task='{}'",
                    self.id, task_id, capability, task
                );

                // 异步执行路由
                let result = self.router
                    .route_with_capability(task, capability, None)
                    .await;

                // 构造结果消息
                let (success, result_value, error_msg) = match result {
                    Ok(route_result) => {
                        info!(
                            "Agent [{}] task [{}] succeeded: skill='{}'",
                            self.id, task_id, route_result.skill.metadata.name
                        );
                        (true, route_result.execution.result.clone(), None)
                    }
                    Err(e) => {
                        error!("Agent [{}] task [{}] failed: {}", self.id, task_id, e);
                        (false, serde_json::Value::Null, Some(e.to_string()))
                    }
                };

                // 回报结果给发送方（或 reply_to 目标）
                let reply_to = if msg.from_agent.is_empty() { "" } else { &msg.from_agent };
                let result_msg = AgentMessage::new(
                    &self.id,
                    reply_to,
                    AgentMessageType::TaskResult {
                        task_id: task_id.clone(),
                        success,
                        result: result_value,
                        error: error_msg,
                    },
                )
                .with_session(&msg.session_id)
                .with_correlation(&msg.message_id);

                bus.publish(result_msg);
            }

            AgentMessageType::TaskCancel { task_id, reason } => {
                warn!(
                    "Agent [{}] received cancel for task [{}]: {}",
                    self.id, task_id, reason
                );
                // MVP 阶段：记录日志，不支持实际取消进行中的任务
            }

            AgentMessageType::HeartBeat { .. } => {
                // 心跳消息：忽略（由 Orchestrator 处理）
                debug!("Agent [{}] received heartbeat from [{}]", self.id, msg.from_agent);
            }

            AgentMessageType::CapabilityAnnouncement { capabilities, role } => {
                debug!(
                    "Agent [{}] noted capability announcement from [{}]: {:?} ({:?})",
                    self.id, msg.from_agent, capabilities, role
                );
            }

            _ => {
                debug!(
                    "Agent [{}] ignoring message type from [{}]",
                    self.id, msg.from_agent
                );
            }
        }
    }
}

impl std::fmt::Debug for AgentRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntime")
            .field("id", &self.id)
            .field("role", &self.role)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}
