pub mod agent_runtime;
pub mod automation;
pub mod builtins;
pub mod config;
pub mod coordinator;
pub mod crew;
pub mod distributed_registry;
pub mod error_kb;
pub mod evolution;
pub mod executor;
pub mod learner;
pub mod loader;
pub mod matcher;
pub mod mcp_client;
pub mod message_bus;
pub mod metrics;
pub mod node_server;
pub mod parallel_executor;
pub mod registry;
pub mod registry_hub;
pub mod security;
pub mod engine_health;
pub mod circuit_breaker;

#[cfg(test)]
mod tests;

use aion_types::ai_native::AiNativePayload;
use aion_types::capability_registry::CapabilityRegistry;
use aion_types::lifecycle::LifecycleRecommendation;
use aion_types::types::{ExecutionContext, ExecutionResponse, RouteResult, RouterPaths, SkillDefinition};
use anyhow::Result;
use tracing::{info, warn};

use aion_intel::online_search::TrustedSourceSearch;
use aion_intel::planner::Planner;
use aion_intel::synth::Synthesizer;

use executor::Executor;
use loader::Loader;
use matcher::Matcher;
use registry::RegistryStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutonomyMode {
    Off,
    Assist,
    Auto,
}

fn autonomy_mode() -> AutonomyMode {
    match std::env::var("AUTONOMY_MODE")
        .unwrap_or_else(|_| "assist".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "off" | "0" | "false" => AutonomyMode::Off,
        "auto" | "2" => AutonomyMode::Auto,
        _ => AutonomyMode::Assist,
    }
}

pub struct SkillRouter {
    paths: RouterPaths,
    capability_registry: std::sync::Mutex<CapabilityRegistry>,
}

impl SkillRouter {
    pub fn new(paths: RouterPaths) -> Result<Self> {
        paths.ensure_base_dirs()?;
        let capability_registry = CapabilityRegistry::load_or_builtin(&paths)?;
        Ok(Self {
            paths,
            capability_registry: std::sync::Mutex::new(capability_registry),
        })
    }

    pub fn paths(&self) -> &RouterPaths {
        &self.paths
    }

    pub fn registry(&self) -> std::sync::MutexGuard<'_, CapabilityRegistry> {
        self.capability_registry.lock().expect("registry lock poisoned")
    }

    pub async fn route(&self, task: &str) -> Result<RouteResult> {
        self.route_with_context(task, None).await
    }

    pub async fn route_with_context(&self, task: &str, context: Option<serde_json::Value>) -> Result<RouteResult> {
        let mode = autonomy_mode();
        let policy = learner::learner().map(|l| l.autonomy_policy());
        let is_blocked = |cap: &str| -> bool {
            policy
                .as_ref()
                .map(|p| p.blocked_capabilities.iter().any(|c| c == cap))
                .unwrap_or(false)
        };

        // Phase 1: 同步关键词推断（持锁，无 await，立即释放）
        let keyword_result = {
            let reg = self
                .capability_registry
                .lock()
                .map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            Planner::infer_via_keywords_only(task, &reg)
        }; // MutexGuard dropped

        if let Some(ref cap) = keyword_result {
            if is_blocked(cap) {
                match mode {
                    AutonomyMode::Auto => {
                        warn!(
                            "autonomy(auto): capability '{}' blocked by unresolved failures, skipping keyword route",
                            cap
                        );
                    }
                    AutonomyMode::Assist => {
                        warn!("autonomy(assist): capability '{}' is blocked suggestion, but will continue fallback phases", cap);
                    }
                    AutonomyMode::Off => return self.route_inner(task, cap, context).await,
                }
            } else {
                return self.route_inner(task, cap, context).await;
            }
        }

        // Phase 2: 异步 AI 推断（锁在独立作用域中，.await 在作用域外）
        let caps_for_ai = {
            let reg = self
                .capability_registry
                .lock()
                .map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            reg.definitions().cloned().collect::<Vec<_>>()
        }; // MutexGuard dropped
        let ai_result = Planner::infer_via_ai_with_defs(task, &caps_for_ai).await;

        if let Some(ref cap) = ai_result {
            if is_blocked(cap) {
                match mode {
                    AutonomyMode::Auto => {
                        warn!(
                            "autonomy(auto): capability '{}' blocked by unresolved failures, skipping ai route",
                            cap
                        );
                    }
                    AutonomyMode::Assist => {
                        warn!("autonomy(assist): capability '{}' is blocked suggestion, but will continue fallback phases", cap);
                    }
                    AutonomyMode::Off => return self.route_inner(task, cap, context).await,
                }
            } else {
                return self.route_inner(task, cap, context).await;
            }
        }

        // Phase 3: AI 发现新能力（需要 &mut registry 写入——获取锁、做同步写、释放）
        let discovered = {
            let mut reg = self
                .capability_registry
                .lock()
                .map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            Planner::ai_discover_sync(task, &mut reg, &self.paths.capabilities_dir)
        };

        if let Some(ref cap) = discovered {
            if is_blocked(cap) && mode == AutonomyMode::Auto {
                return Err(anyhow::anyhow!(
                    "autonomy policy blocked discovered capability '{}' due to unresolved failures",
                    cap
                ));
            }
            return self.route_inner(task, cap, context).await;
        }

        if mode != AutonomyMode::Off {
            if let Some(p) = policy {
                if !p.preferred_capabilities.is_empty() {
                    warn!(
                        "autonomy: no route inferred for task '{}'; preferred capabilities: {:?}",
                        task, p.preferred_capabilities
                    );
                }
            }
        }

        Err(anyhow::anyhow!("could not infer capability for task: '{task}'"))
    }

    pub async fn route_with_capability(
        &self,
        task: &str,
        capability: &str,
        context: Option<serde_json::Value>,
    ) -> Result<RouteResult> {
        {
            let reg = self
                .capability_registry
                .lock()
                .map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            reg.validate_name(capability)
                .map_err(|_| anyhow::anyhow!("unknown capability: {capability}"))?;
        }
        self.route_inner(task, capability, context).await
    }

    async fn route_inner(
        &self,
        task: &str,
        capability: &str,
        extra_context: Option<serde_json::Value>,
    ) -> Result<RouteResult> {
        // 获取 registry 锁内的同步数据，然后立即释放锁（不跨 await 点持有 MutexGuard）
        let (matching_local, trusted, registry_store) = {
            let reg = self
                .capability_registry
                .lock()
                .map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            let local_skills = Loader::load_local_skills(&self.paths, &reg)?;
            let matching: Vec<SkillDefinition> = local_skills
                .into_iter()
                .filter(|skill| skill.supports_capability(capability))
                .collect();
            let trusted = TrustedSourceSearch::search(&self.paths, capability).unwrap_or_default();
            let registry_store = RegistryStore::load(&self.paths)?;
            (matching, trusted, registry_store)
        }; // MutexGuard dropped here — safe to .await below

        let learner_ref = learner::learner();
        let selected = if !matching_local.is_empty() {
            // Tier 1: Local Match
            Matcher::select_best_full(
                capability,
                &matching_local,
                &trusted,
                Some(&registry_store),
                learner_ref,
            )?
        } else {
            // Tier 2: Cascade Discovery
            info!(
                "Local skill miss — triggering DiscoveryRadar cascade search for '{}'",
                task
            );
            let discovery = aion_intel::discovery_radar::DiscoveryRadar::cascade_search(task, &self.paths).await?;

            // Tier 3: Synthesis with registry-aware context
            info!("Synthesizing new skill '{}' with discovery context", capability);
            let discovery_json = aion_intel::discovery_radar::DiscoveryRadar::to_json(&discovery);
            let synthesized = {
                let reg = self
                    .capability_registry
                    .lock()
                    .map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
                Synthesizer::create_placeholder_with_context(
                    &self.paths,
                    capability,
                    task,
                    Some(discovery_json),
                    Some(&reg),
                )?
            };

            Matcher::select_best_full(capability, &[synthesized], &trusted, Some(&registry_store), learner_ref)?
        };

        let exec_ctx = {
            let mut ctx = ExecutionContext::new(task, capability);
            if let Some(extra) = extra_context {
                ctx = ctx.with_context(extra);
            }
            ctx
        };

        let execution = Executor::execute(&selected, &exec_ctx, &self.paths).await?;

        let mut registry = RegistryStore::load(&self.paths)?;
        let success = execution.status == "ok";
        registry.record_execution(&selected.metadata.name, success, std::time::SystemTime::now());
        registry.save(&self.paths)?;

        // Write execution results to memory system for cross-session learning
        Self::record_execution_to_memory(&self.paths, capability, &selected.metadata.name, &execution);

        let stats = registry
            .skill_stats(&selected.metadata.name)
            .ok_or_else(|| anyhow::anyhow!("missing registry stats for {}", selected.metadata.name))?;
        let lifecycle = LifecycleRecommendation::from_stats(&stats, std::time::SystemTime::now());

        Ok(RouteResult {
            capability: capability.to_string(),
            skill: selected,
            execution,
            lifecycle,
        })
    }

    /// AI-native entry-point: accept a structured payload instead of natural language.
    pub async fn route_native(&self, payload: AiNativePayload) -> Result<RouteResult> {
        let primary_result = {
            let ctx = payload.to_execution_context();
            match &payload.capability {
                Some(cap) => self.route_inner(&ctx.task, cap, Some(ctx.context.clone())).await,
                None => self.route_with_context(&ctx.task, Some(ctx.context.clone())).await,
            }
        };

        match primary_result {
            Ok(res) if res.execution.status == "ok" => Ok(res),
            _ => match payload.autonomous.recovery_strategy {
                aion_types::ai_native::RecoveryStrategy::ReSynthesize => {
                    warn!(
                        "Primary execution failed — autonomous recovery: re-synthesizing skill for '{}'",
                        payload.intent
                    );
                    let discovery =
                        aion_intel::discovery_radar::DiscoveryRadar::cascade_search(&payload.intent, &self.paths)
                            .await?;
                    let discovery_json = aion_intel::discovery_radar::DiscoveryRadar::to_json(&discovery);

                    let capability = payload.capability.clone().unwrap_or_else(|| payload.intent.clone());
                    let _recovered_skill = {
                        let reg = self
                            .capability_registry
                            .lock()
                            .map_err(|e| anyhow::anyhow!("lock: {}", e))?;
                        Synthesizer::create_placeholder_with_context(
                            &self.paths,
                            &capability,
                            &payload.intent,
                            Some(discovery_json),
                            Some(&reg),
                        )?
                    };

                    info!("Retrying execution with re-synthesized skill for '{}'", payload.intent);
                    let ctx = payload.to_execution_context();
                    match &payload.capability {
                        Some(cap) => self.route_inner(&ctx.task, cap, Some(ctx.context.clone())).await,
                        None => self.route_with_context(&ctx.task, Some(ctx.context.clone())).await,
                    }
                }
                aion_types::ai_native::RecoveryStrategy::Fallback(ref cap) => {
                    warn!(
                        "Primary execution failed — autonomous recovery: falling back to capability '{}'",
                        cap
                    );
                    self.route_inner(&payload.intent, cap, Some(payload.parameters.clone()))
                        .await
                }
                aion_types::ai_native::RecoveryStrategy::None => primary_result,
                aion_types::ai_native::RecoveryStrategy::AgentFailover {
                    ref preferred,
                    ref fallback_agents,
                } => {
                    // 升级（P3）：distributed feature 下，若存在 NATS 连接则向备份 agents
                    // 发送任务请求实现跨节点 failover；否则（或无 NATS/超时）保留同进程降级。
                    #[cfg(feature = "distributed")]
                    {
                        let mut targets = fallback_agents.clone();
                        if !targets.iter().any(|t| t == preferred) {
                            targets.insert(0, preferred.clone());
                        }
                        if let Some(result) = self.agent_failover_nats(&payload, &targets).await? {
                            return Ok(result);
                        }
                        warn!(
                            "AgentFailover: NATS unavailable or no agent replied, falling back to in-process execution"
                        );
                    }
                    let fallback_cap = fallback_agents
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or(preferred.as_str());
                    warn!(
                        "AgentFailover: in-process fallback to capability '{}'",
                        fallback_cap
                    );
                    self.route_inner(&payload.intent, fallback_cap, Some(payload.parameters.clone()))
                        .await
                }
            },
        }
    }

    /// AgentFailover（distributed）：通过 NATS 向备份 agents 发送任务请求实现跨节点 failover。
    ///
    /// 只读全局消息总线的 NATS 后端：
    /// - 无 NATS 连接 → 返回 `Ok(None)`，调用方降级到同进程执行；
    /// - 向每个目标 agent 的寻址 subject `aion.agents.{agent_id}.tasks` 发布
    ///   `AgentMessage::TaskAssignment`（request-reply，correlation_id = task_id），
    ///   然后订阅 `aion.results.{task_id}` 等待任一节点回复 `TaskResult`。
    /// - 30s 内无人回复或执行失败 → 返回 `Ok(None)` 让调用方降级。
    ///
    /// 接收端（agent 侧 worker）需订阅 `aion.agents.*.tasks`、执行任务并向
    /// `aion.results.{task_id}` 回复 TaskResult——该 worker 不属于本 crate，
    /// 属于部署侧的 agent 运行时（参见 `message_bus::nats_subjects`）。
    #[cfg(feature = "distributed")]
    async fn agent_failover_nats(
        &self,
        payload: &AiNativePayload,
        targets: &[String],
    ) -> Result<Option<RouteResult>> {
        use aion_types::agent_message::{AgentMessage, AgentMessageType};
        use aion_types::lifecycle::LifecycleRecommendation;
        use aion_types::types::{PermissionSet, SkillMetadata, SkillSource};
        use futures_util::StreamExt;

        // 通过全局消息总线获取 NATS 后端（无连接 → 降级同进程执行）
        let bus = crate::message_bus::global_message_bus();
        let Some(nats) = bus.nats_backend() else {
            warn!("AgentFailover: no NATS connection, falling back to in-process execution");
            return Ok(None);
        };
        if targets.is_empty() {
            return Ok(None);
        }

        let capability = payload.capability.clone().unwrap_or_else(|| payload.intent.clone());
        let task_id = uuid::Uuid::new_v4().to_string();
        let reply_subject = format!("aion.results.{}", task_id);
        let client = nats.client();

        // 向每个 fallback agent 的寻址 subject 发布 TaskAssignment（request-reply 模式）
        for agent in targets {
            let subject = format!("aion.agents.{}.tasks", agent);
            let msg = AgentMessage::new("aion-router", agent, AgentMessageType::TaskAssignment {
                task_id: task_id.clone(),
                task: payload.intent.clone(),
                capability: capability.clone(),
            })
            .with_session(&payload.metadata.session_id)
            .with_correlation(&task_id);
            let bytes = serde_json::to_vec(&msg)?;
            client.publish(subject, bytes.into()).await?;
            info!(
                "AgentFailover: task '{}' delegated to agent '{}' via NATS",
                task_id, agent
            );
        }

        // 订阅 reply subject，等待任一节点返回 TaskResult
        let mut subscriber = match client.subscribe(reply_subject.clone()).await {
            Ok(sub) => sub,
            Err(e) => {
                warn!("AgentFailover: failed to subscribe '{}': {}", reply_subject, e);
                return Ok(None);
            }
        };
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while let Some(nats_msg) = subscriber.next().await {
                let Ok(agent_msg) = serde_json::from_slice::<AgentMessage>(&nats_msg.payload) else {
                    continue;
                };
                match agent_msg.message_type {
                    AgentMessageType::TaskResult {
                        task_id: t,
                        success,
                        result,
                        error,
                    } if t == task_id => return Some((success, result, error)),
                    _ => {}
                }
            }
            None
        })
        .await;

        let Ok(Some((success, result, error))) = outcome else {
            warn!("AgentFailover: no agent replied for task '{}' within timeout", task_id);
            return Ok(None);
        };

        info!("AgentFailover: remote execution for '{}' success={}", task_id, success);
        // 构造代表远程执行的 RouteResult（skill 为 RemoteCandidate 占位，保留真实执行结果）
        let status = if success { "ok" } else { "error" };
        let metadata = SkillMetadata {
            name: format!("remote_agent.{}", task_id),
            version: "0.1.0".to_string(),
            capabilities: vec![capability.clone()],
            entrypoint: "builtin:ai_task".to_string(),
            permissions: PermissionSet::default(),
            instruction: None,
            engine_capable: false,
        };
        let skill = SkillDefinition {
            metadata,
            root_dir: self.paths.skills_dir.clone(),
            source: SkillSource::RemoteCandidate,
        };
        let execution = ExecutionResponse {
            status: status.to_string(),
            result,
            artifacts: serde_json::Value::Object(Default::default()),
            error,
            token_usage: None,
        };
        Ok(Some(RouteResult {
            capability,
            skill,
            execution,
            lifecycle: LifecycleRecommendation::Observe,
        }))
    }

    /// Record significant execution results to the memory system for cross-session learning.
    /// Failures are always recorded (Error category). Successes are only recorded when
    /// they represent a first successful execution of a capability (Lesson category).
    fn record_execution_to_memory(
        paths: &RouterPaths,
        capability: &str,
        skill_name: &str,
        execution: &ExecutionResponse,
    ) {
        use aion_memory::memory::{MemoryCategory, MemoryManager};

        let mem = MemoryManager::new(&paths.workspace_root);

        if execution.status != "ok" {
            // Always record failures — they are valuable lessons
            let error_msg = execution.error.as_deref().unwrap_or("unknown error");
            let content = format!(
                "Skill '{}' (capability '{}') failed: {}",
                skill_name, capability, error_msg
            );
            if let Err(e) = mem.remember(MemoryCategory::Error, &content, "system", 7) {
                warn!("Failed to write execution error to memory: {}", e);
            }
        } else {
            // Only record first-time successes for a capability (avoid flooding memory)
            if let Some(learner) = crate::learner::learner() {
                let stats = learner.get_stats(capability);
                // Record lesson only on first successful execution (total == 1 means just recorded)
                if stats.map(|s| s.ok == 1).unwrap_or(false) {
                    let content = format!(
                        "Capability '{}' first successful execution via skill '{}'",
                        capability, skill_name
                    );
                    if let Err(e) = mem.remember(MemoryCategory::Lesson, &content, "system", 4) {
                        warn!("Failed to write execution lesson to memory: {}", e);
                    }
                }
            }
        }
    }
}
