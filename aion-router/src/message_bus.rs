//! 消息总线 — 支持 local（进程内）和 distributed（NATS）两种后端
//!
//! 默认使用 `local` feature（tokio broadcast channel）。
//! 启用 `distributed` feature 后切换为 NATS JetStream 后端，
//! 接口完全一致，上层代码零改动。
//!
//! ```toml
//! # Cargo.toml
//! [features]
//! default = ["local"]
//! distributed = ["async-nats", "axum", "tower-http"]
//! ```

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn};
#[cfg(feature = "distributed")]
use tracing::info;

use aion_types::agent_message::AgentMessage;

// ══════════════════════════════════════════════════════════════════════════════
// Local backend (default) — 进程内 tokio broadcast
// ══════════════════════════════════════════════════════════════════════════════

/// 进程内消息总线（local 后端）
///
/// 底层使用 `tokio::sync::broadcast::channel`，支持多生产者多消费者。
/// 所有 Agent 订阅同一 channel，根据 `to_agent` 字段自行过滤消息。
#[derive(Clone)]
pub struct MessageBus {
    tx: Arc<broadcast::Sender<AgentMessage>>,
    /// NATS 连接（仅 distributed feature 启用时有值）
    #[cfg(feature = "distributed")]
    nats: Option<NatsBackend>,
}

impl MessageBus {
    /// 创建消息总线，指定 channel 容量（建议 64-256）
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self {
            tx: Arc::new(tx),
            #[cfg(feature = "distributed")]
            nats: None,
        }
    }

    /// 发布消息（广播给所有本地订阅者）
    ///
    /// 返回实际接收到消息的订阅者数量
    pub fn publish(&self, msg: AgentMessage) -> usize {
        // 同时发布到 NATS（如果已连接）
        #[cfg(feature = "distributed")]
        if let Some(ref nats) = self.nats {
            let msg_clone = msg.clone();
            let nats_clone = nats.clone();
            tokio::spawn(async move {
                if let Err(e) = nats_clone.publish(&msg_clone).await {
                    warn!("NATS publish failed: {}", e);
                }
            });
        }

        match self.tx.send(msg) {
            Ok(count) => {
                debug!("Message published to {} local subscriber(s)", count);
                count
            }
            Err(_) => {
                warn!("Message published but no active local subscribers");
                0
            }
        }
    }

    /// 订阅消息总线（本地 channel），返回接收端
    pub fn subscribe(&self) -> broadcast::Receiver<AgentMessage> {
        self.tx.subscribe()
    }

    /// 当前活跃本地订阅者数量
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl std::fmt::Debug for MessageBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageBus {{ local_subscribers: {}", self.subscriber_count())?;
        #[cfg(feature = "distributed")]
        write!(f, ", nats: {}", if self.nats.is_some() { "connected" } else { "disconnected" })?;
        write!(f, " }}")
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// NATS backend (distributed feature)
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "distributed")]
mod nats_backend {
    use super::*;
    use anyhow::Result;
    use async_nats::Client as NatsClient;

    /// NATS subject 命名规范
    pub mod subjects {
        /// 任务分发队列（queue group 保证只投递一个订阅者）
        pub fn task(capability: &str) -> String {
            format!("aion.tasks.{}", capability)
        }
        /// 结果汇报通道
        pub fn result(session_id: &str) -> String {
            format!("aion.results.{}", session_id)
        }
        /// 广播通道（所有节点接收）
        pub fn broadcast() -> &'static str {
            "aion.broadcast"
        }
        /// Agent 心跳通道
        pub fn heartbeat() -> &'static str {
            "aion.heartbeat"
        }
    }

    /// NATS 后端封装
    #[derive(Clone)]
    pub struct NatsBackend {
        client: NatsClient,
    }

    impl NatsBackend {
        /// 连接 NATS 服务器
        pub async fn connect(url: &str) -> Result<Self> {
            info!("Connecting to NATS at {}", url);
            let client = async_nats::connect(url).await?;
            info!("NATS connected successfully");
            Ok(Self { client })
        }

        /// 发布 AgentMessage 到 NATS
        pub async fn publish(&self, msg: &AgentMessage) -> Result<()> {
            let payload = serde_json::to_vec(msg)?;
            let subject = if msg.is_broadcast() {
                subjects::broadcast().to_string()
            } else {
                // 按消息类型路由到不同 subject
                match &msg.message_type {
                    aion_types::agent_message::AgentMessageType::TaskAssignment { capability, .. } => {
                        subjects::task(capability)
                    }
                    aion_types::agent_message::AgentMessageType::TaskResult { .. } => {
                        subjects::result(&msg.session_id)
                    }
                    _ => subjects::broadcast().to_string(),
                }
            };
            self.client.publish(subject.clone(), payload.into()).await?;
            debug!("NATS: published to subject '{}'", subject);
            Ok(())
        }

        /// 订阅指定 subject，返回消息流
        pub async fn subscribe_subject(&self, subject: &str) -> Result<async_nats::Subscriber> {
            let sub = self.client.subscribe(subject.to_string()).await?;
            Ok(sub)
        }

        /// 获取 NATS 客户端引用（用于 JetStream KV 等高级操作）
        pub fn client(&self) -> &NatsClient {
            &self.client
        }
    }

    impl MessageBus {
        /// 创建带 NATS 后端的消息总线
        ///
        /// 同时保留本地 broadcast channel（用于进程内路由），
        /// NATS 用于跨节点通信。
        pub async fn with_nats(capacity: usize, nats_url: &str) -> Result<Self> {
            let (tx, _rx) = broadcast::channel(capacity);
            let nats = NatsBackend::connect(nats_url).await?;
            Ok(Self {
                tx: Arc::new(tx),
                nats: Some(nats),
            })
        }

        /// 获取 NATS 后端引用
        pub fn nats_backend(&self) -> Option<&NatsBackend> {
            self.nats.as_ref()
        }

        /// 启动 NATS → 本地 bridge（将 NATS 收到的消息转发到本地 broadcast）
        ///
        /// 返回 JoinHandle，在后台持续运行
        pub fn spawn_nats_bridge(&self, subject: &str) -> Option<tokio::task::JoinHandle<()>> {
            let nats = self.nats.clone()?;
            let tx = Arc::clone(&self.tx);
            let subject = subject.to_string();

            Some(tokio::spawn(async move {
                match nats.subscribe_subject(&subject).await {
                    Ok(mut subscriber) => {
                        info!("NATS bridge started for subject '{}'", subject);
                        while let Some(nats_msg) = subscriber.next().await {
                            match serde_json::from_slice::<AgentMessage>(&nats_msg.payload) {
                                Ok(agent_msg) => {
                                    debug!("NATS bridge: forwarding message from [{}]", agent_msg.from_agent);
                                    let _ = tx.send(agent_msg);
                                }
                                Err(e) => {
                                    warn!("NATS bridge: failed to deserialize message: {}", e);
                                }
                            }
                        }
                        info!("NATS bridge stopped for subject '{}'", subject);
                    }
                    Err(e) => {
                        tracing::error!("NATS bridge: failed to subscribe to '{}': {}", subject, e);
                    }
                }
            }))
        }
    }
}

#[cfg(feature = "distributed")]
pub use nats_backend::{NatsBackend, subjects as nats_subjects};

// Re-export for bridge subscriber iteration
#[cfg(feature = "distributed")]
use futures_util::StreamExt;
