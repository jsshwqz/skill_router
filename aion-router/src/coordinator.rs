//! 多 Agent 协调器
//!
//! `MultiAgentCoordinator` 提供四种多 Agent 工作流模式：
//!
//! 1. **串行委派**（`process_serial_pipeline`）：A → B → C，每步结果注入下一步
//! 2. **并行分工**（`process_task_parallel`）：任务分解为 DAG，并行发送给多个 Agent
//! 3. **专家会议**（`consult_experts`）：广播同一问题，等待所有 Agent 回应后聚合
//! 4. **竞争执行**（`process_competitive`）：多 Agent 竞争，取第一个成功结果
//!
//! # MVP 阶段说明
//! 当前实现使用进程内 `MessageBus`（tokio broadcast channel），
//! Phase 1（D1）将切换为 NATS 后端，接口保持不变。

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use aion_types::agent_message::{AgentMessage, AgentMessageType, AgentRef};
use aion_types::capability_registry::CapabilityRegistry;
use aion_types::parallel::{ParallelResponse, TaskGraph};
use aion_types::types::RouterPaths;

use crate::message_bus::MessageBus;
use crate::parallel_executor::ParallelExecutor;
use aion_intel::parallel_planner::ParallelPlanner;

// ── 协调器结构 ────────────────────────────────────────────────────────────────

/// 多 Agent 协调器
///
/// 持有对消息总线的共享引用，管理 Agent 注册表，
/// 提供四种工作流模式的入口函数。
pub struct MultiAgentCoordinator {
    /// 进程内消息总线
    bus: Arc<MessageBus>,
    /// 已注册的 Agent 列表（id → AgentRef）
    agents: HashMap<String, AgentRef>,
    /// 任务等待超时时间
    task_timeout: Duration,
}

impl MultiAgentCoordinator {
    /// 创建协调器，绑定消息总线
    pub fn new(bus: Arc<MessageBus>) -> Self {
        Self {
            bus,
            agents: HashMap::new(),
            task_timeout: Duration::from_secs(30),
        }
    }

    /// 注册一个 Agent
    pub fn register_agent(&mut self, agent: AgentRef) {
        info!("Coordinator: registered agent [{}] with role {:?}", agent.id, agent.role);
        self.agents.insert(agent.id.clone(), agent);
    }

    /// 设置任务超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.task_timeout = timeout;
        self
    }

    // ── 模式一：串行委派管道 ──────────────────────────────────────────────────

    /// 串行委派管道：将任务序列按顺序发给 Agent，每步输出注入下一步
    ///
    /// 适用场景：多步骤数据处理（如 fetch → summarize → translate）
    ///
    /// # 参数
    /// - `steps`：每步的 `(capability, task_description)` 元组列表
    /// - `orchestrator_id`：发起方 Agent ID
    /// - `target_agent_id`：可选，指定执行 Agent（None = 广播给最优 Agent）
    ///
    /// # 返回
    /// 最后一步的执行结果
    pub async fn process_serial_pipeline(
        &self,
        steps: Vec<(String, String)>, // Vec<(capability, task)>
        orchestrator_id: &str,
        target_agent_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut upstream_result: Option<serde_json::Value> = None;

        for (step_idx, (capability, task)) in steps.iter().enumerate() {
            info!(
                "Pipeline step {}/{}: capability='{}', task='{}'",
                step_idx + 1,
                steps.len(),
                capability,
                task
            );

            // 将上游结果注入任务描述（如果有）
            let effective_task = if let Some(ref upstream) = upstream_result {
                format!("{}\n\nContext from previous step:\n{}", task, upstream)
            } else {
                task.clone()
            };

            let task_id = uuid::Uuid::new_v4().to_string();
            let to = target_agent_id.unwrap_or("");

            let result = self
                .send_task_and_wait(&task_id, orchestrator_id, to, &effective_task, capability)
                .await?;

            upstream_result = Some(result);
        }

        upstream_result.ok_or_else(|| anyhow!("Pipeline had no steps"))
    }

    // ── 模式二：并行分工（对外保留原有接口）───────────────────────────────────

    /// 并行任务图执行（向后兼容原有 `MultiSkillCoordinator::process_task_parallel`）
    ///
    /// 任务通过 AI 分解为 DAG，依赖满足后并行执行。
    /// 当前 MVP 使用 tokio JoinSet；D1 阶段升级为 NATS 跨节点分发。
    pub async fn process_task_parallel(
        task: &str,
        paths: &RouterPaths,
        reg: &CapabilityRegistry,
    ) -> Result<ParallelResponse> {
        info!("Coordinator: splitting task into parallel graph — '{}'", task);
        let graph = ParallelPlanner::split_task(task, paths).await?;
        ParallelExecutor::execute_graph(graph, paths, reg).await
    }

    /// 直接执行预构建的任务图
    pub async fn execute_graph(
        graph: TaskGraph,
        paths: &RouterPaths,
        reg: &CapabilityRegistry,
    ) -> Result<ParallelResponse> {
        ParallelExecutor::execute_graph(graph, paths, reg).await
    }

    // ── 模式三：专家会议 ──────────────────────────────────────────────────────

    /// 专家会议：向所有具有指定能力的 Agent 广播同一问题，聚合所有回应
    ///
    /// 适用场景：代码审查（多个 Reviewer 同时审）、安全评估（多个 Specialist 评分）
    ///
    /// # 参数
    /// - `capability`：需要的能力
    /// - `task`：任务描述
    /// - `orchestrator_id`：发起方 Agent ID
    ///
    /// # 返回
    /// 所有 Agent 的回应列表（按到达顺序）
    pub async fn consult_experts(
        &self,
        capability: &str,
        task: &str,
        orchestrator_id: &str,
    ) -> Result<Vec<ExpertOpinion>> {
        // 查找所有支持该能力的 Agent
        let expert_agents: Vec<&AgentRef> = self
            .agents
            .values()
            .filter(|a| a.supports_capability(capability))
            .collect();

        if expert_agents.is_empty() {
            return Err(anyhow!("No agents available for capability '{}'", capability));
        }

        info!(
            "Expert panel: consulting {} agents for capability '{}'",
            expert_agents.len(),
            capability
        );

        let mut rx = self.bus.subscribe();
        let task_id_prefix = uuid::Uuid::new_v4().to_string();
        let mut pending_ids = std::collections::HashSet::new();

        // 向每个专家发送任务
        for agent in &expert_agents {
            let task_id = format!("{}-{}", task_id_prefix, agent.id);
            pending_ids.insert(task_id.clone());

            let msg = AgentMessage::new(
                orchestrator_id,
                &agent.id,
                AgentMessageType::TaskAssignment {
                    task_id,
                    task: task.to_string(),
                    capability: capability.to_string(),
                },
            );
            self.bus.publish(msg);
        }

        // 收集所有专家意见
        let mut opinions = Vec::new();
        let deadline = tokio::time::Instant::now() + self.task_timeout;

        while !pending_ids.is_empty() && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(msg)) => {
                    if let AgentMessageType::TaskResult { task_id, success, result, error } =
                        &msg.message_type
                    {
                        if pending_ids.remove(task_id) {
                            opinions.push(ExpertOpinion {
                                agent_id: msg.from_agent.clone(),
                                task_id: task_id.clone(),
                                success: *success,
                                result: result.clone(),
                                error: error.clone(),
                            });
                            debug!(
                                "Received expert opinion from [{}] (remaining: {})",
                                msg.from_agent,
                                pending_ids.len()
                            );
                        }
                    }
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("Expert panel coordinator lagged, skipped {} messages", n);
                }
                _ => break,
            }
        }

        if pending_ids.len() > 0 {
            warn!(
                "Expert panel timed out: {} agents did not respond: {:?}",
                pending_ids.len(),
                pending_ids
            );
        }

        Ok(opinions)
    }

    // ── 模式四：竞争执行 ──────────────────────────────────────────────────────

    /// 竞争执行：向多个 Agent 同时发送同一任务，取第一个成功结果
    ///
    /// 适用场景：对延迟敏感的任务（如实时搜索、LLM 推理）
    ///
    /// # 参数
    /// - `capability`：需要的能力
    /// - `task`：任务描述
    /// - `orchestrator_id`：发起方 Agent ID
    ///
    /// # 返回
    /// 第一个成功 Agent 的执行结果
    pub async fn process_competitive(
        &self,
        capability: &str,
        task: &str,
        orchestrator_id: &str,
    ) -> Result<serde_json::Value> {
        let candidates: Vec<&AgentRef> = self
            .agents
            .values()
            .filter(|a| a.supports_capability(capability))
            .collect();

        if candidates.is_empty() {
            return Err(anyhow!("No agents available for capability '{}'", capability));
        }

        info!(
            "Competitive execution: {} candidates for capability '{}'",
            candidates.len(),
            capability
        );

        let mut rx = self.bus.subscribe();
        let task_id_base = uuid::Uuid::new_v4().to_string();
        let mut pending_ids = std::collections::HashSet::new();

        // 向所有候选 Agent 发送任务
        for agent in &candidates {
            let task_id = format!("{}-{}", task_id_base, agent.id);
            pending_ids.insert(task_id.clone());

            let msg = AgentMessage::new(
                orchestrator_id,
                &agent.id,
                AgentMessageType::TaskAssignment {
                    task_id,
                    task: task.to_string(),
                    capability: capability.to_string(),
                },
            );
            self.bus.publish(msg);
        }

        // 等待第一个成功结果
        let deadline = tokio::time::Instant::now() + self.task_timeout;
        let mut winner: Option<serde_json::Value> = None;
        let winner_agent;

        while winner.is_none() && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(msg)) => {
                    if let AgentMessageType::TaskResult { task_id, success, result, .. } =
                        &msg.message_type
                    {
                        if pending_ids.contains(task_id) && *success {
                            winner = Some(result.clone());
                            winner_agent = msg.from_agent.clone();
                            let _ = &winner_agent; // suppress unused warning until cancel loop
                            info!(
                                "Competitive winner: agent [{}] for task_id='{}'",
                                winner_agent, task_id
                            );

                            // 通知其他 Agent 取消任务
                            for cancel_id in pending_ids.iter().filter(|id| *id != task_id) {
                                let cancel_msg = AgentMessage::new(
                                    orchestrator_id,
                                    "", // 广播
                                    AgentMessageType::TaskCancel {
                                        task_id: cancel_id.clone(),
                                        reason: format!("Competitive winner: {}", winner_agent),
                                    },
                                );
                                self.bus.publish(cancel_msg);
                            }
                            break;
                        }
                    }
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("Competitive coordinator lagged, skipped {} messages", n);
                }
                _ => break,
            }
        }

        winner.ok_or_else(|| anyhow!("Competitive execution: no agent completed successfully within timeout"))
    }

    // ── 内部工具函数 ──────────────────────────────────────────────────────────

    /// 发送任务并等待结果（单 Agent 调用的基础函数）
    async fn send_task_and_wait(
        &self,
        task_id: &str,
        from: &str,
        to: &str,
        task: &str,
        capability: &str,
    ) -> Result<serde_json::Value> {
        let mut rx = self.bus.subscribe();

        let msg = AgentMessage::new(
            from,
            to,
            AgentMessageType::TaskAssignment {
                task_id: task_id.to_string(),
                task: task.to_string(),
                capability: capability.to_string(),
            },
        );
        self.bus.publish(msg);

        let deadline = tokio::time::Instant::now() + self.task_timeout;
        let task_id_owned = task_id.to_string();

        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(msg)) => {
                    if let AgentMessageType::TaskResult { task_id: tid, success, result, error } =
                        &msg.message_type
                    {
                        if tid == &task_id_owned {
                            return if *success {
                                Ok(result.clone())
                            } else {
                                Err(anyhow!(
                                    "Agent [{}] task [{}] failed: {}",
                                    msg.from_agent,
                                    tid,
                                    error.as_deref().unwrap_or("unknown error")
                                ))
                            };
                        }
                    }
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!("Task coordinator lagged, skipped {} messages", n);
                }
                _ => {
                    return Err(anyhow!(
                        "Task [{}] timed out after {:?}",
                        task_id_owned,
                        self.task_timeout
                    ));
                }
            }
        }
    }
}

// ── 专家意见结构 ──────────────────────────────────────────────────────────────

/// 单个 Agent 对专家会议问题的回应
#[derive(Debug, Clone)]
pub struct ExpertOpinion {
    /// 回应的 Agent ID
    pub agent_id: String,
    /// 对应的任务 ID
    pub task_id: String,
    /// 是否成功
    pub success: bool,
    /// 执行结果
    pub result: serde_json::Value,
    /// 失败时的错误信息
    pub error: Option<String>,
}

impl ExpertOpinion {
    /// 多数表决：返回成功 Agent 的比例
    pub fn success_rate(opinions: &[ExpertOpinion]) -> f32 {
        if opinions.is_empty() {
            return 0.0;
        }
        let success_count = opinions.iter().filter(|o| o.success).count();
        success_count as f32 / opinions.len() as f32
    }

    /// 取多数票结果（返回最多 Agent 认同的成功结果）
    pub fn majority_result(opinions: &[ExpertOpinion]) -> Option<&serde_json::Value> {
        opinions
            .iter()
            .filter(|o| o.success)
            .max_by_key(|_| 1) // MVP 简化：取第一个成功结果
            .map(|o| &o.result)
    }
}
