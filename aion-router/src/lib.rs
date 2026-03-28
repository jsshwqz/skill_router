pub mod config;
pub mod executor;
pub mod builtins;
pub mod metrics;
pub mod learner;
pub mod registry;
pub mod loader;
pub mod matcher;
pub mod security;
pub mod parallel_executor;
pub mod coordinator;
pub mod automation;
pub mod message_bus;
pub mod agent_runtime;
pub mod node_server;
pub mod distributed_registry;
pub mod registry_hub;
pub mod mcp_client;
pub mod crew;

#[cfg(test)]
mod tests;

use anyhow::Result;
use tracing::{info, warn};
use aion_types::capability_registry::CapabilityRegistry;
use aion_types::lifecycle::LifecycleRecommendation;
use aion_types::types::{ExecutionContext, RouteResult, RouterPaths, SkillDefinition};
use aion_types::ai_native::AiNativePayload;

use aion_intel::planner::Planner;
use aion_intel::synth::Synthesizer;
use aion_intel::online_search::TrustedSourceSearch;

use executor::Executor;
use loader::Loader;
use matcher::Matcher;
use registry::RegistryStore;

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
        // Phase 1: 同步关键词推断（持锁，无 await，立即释放）
        let keyword_result = {
            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            Planner::infer_via_keywords_only(task, &reg)
        }; // MutexGuard dropped

        if let Some(ref cap) = keyword_result {
            return self.route_inner(task, cap, context).await;
        }

        // Phase 2: 异步 AI 推断（锁在独立作用域中，.await 在作用域外）
        let caps_for_ai = {
            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            reg.definitions().cloned().collect::<Vec<_>>()
        }; // MutexGuard dropped
        let ai_result = Planner::infer_via_ai_with_defs(task, &caps_for_ai).await;

        if let Some(ref cap) = ai_result {
            return self.route_inner(task, cap, context).await;
        }

        // Phase 3: AI 发现新能力（需要 &mut registry 写入——获取锁、做同步写、释放）
        let discovered = {
            let mut reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            Planner::ai_discover_sync(task, &mut reg, &self.paths.capabilities_dir)
        };

        if let Some(ref cap) = discovered {
            return self.route_inner(task, cap, context).await;
        }

        Err(anyhow::anyhow!("could not infer capability for task: '{task}'"))
    }

    pub async fn route_with_capability(&self, task: &str, capability: &str, context: Option<serde_json::Value>) -> Result<RouteResult> {
        {
            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            reg.validate_name(capability)
                .map_err(|_| anyhow::anyhow!("unknown capability: {capability}"))?;
        }
        self.route_inner(task, capability, context).await
    }

    async fn route_inner(&self, task: &str, capability: &str, extra_context: Option<serde_json::Value>) -> Result<RouteResult> {
        // 获取 registry 锁内的同步数据，然后立即释放锁（不跨 await 点持有 MutexGuard）
        let (matching_local, trusted, registry_store) = {
            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            let local_skills = Loader::load_local_skills(&self.paths, &reg)?;
            let matching: Vec<SkillDefinition> = local_skills
                .into_iter()
                .filter(|skill| skill.supports_capability(capability))
                .collect();
            let trusted = TrustedSourceSearch::search(&self.paths, capability).unwrap_or_default();
            let registry_store = RegistryStore::load(&self.paths)?;
            (matching, trusted, registry_store)
        }; // MutexGuard dropped here — safe to .await below

        let selected = if !matching_local.is_empty() {
            // Tier 1: Local Match
            Matcher::select_best_with_registry(capability, &matching_local, &trusted, Some(&registry_store))?
        } else {
            // Tier 2: Cascade Discovery
            info!("Local skill miss — triggering DiscoveryRadar cascade search for '{}'", task);
            let discovery = aion_intel::discovery_radar::DiscoveryRadar::cascade_search(task, &self.paths).await?;

            // Tier 3: Synthesis with registry-aware context
            info!("Synthesizing new skill '{}' with discovery context", capability);
            let discovery_json = aion_intel::discovery_radar::DiscoveryRadar::to_json(&discovery);
            let synthesized = {
                let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
                Synthesizer::create_placeholder_with_context(
                    &self.paths,
                    capability,
                    task,
                    Some(discovery_json),
                    Some(&reg),
                )?
            };

            Matcher::select_best_with_registry(capability, &[synthesized], &trusted, Some(&registry_store))?
        };

        let exec_ctx = {
            let mut ctx = ExecutionContext::new(task, capability);
            if let Some(extra) = extra_context { ctx = ctx.with_context(extra); }
            ctx
        };

        let execution = Executor::execute(&selected, &exec_ctx, &self.paths).await?;

        let mut registry = RegistryStore::load(&self.paths)?;
        registry.record_execution(&selected.metadata.name, execution.status == "ok", std::time::SystemTime::now());
        registry.save(&self.paths)?;

        let stats = registry.skill_stats(&selected.metadata.name)
            .ok_or_else(|| anyhow::anyhow!("missing registry stats for {}", selected.metadata.name))?;
        let lifecycle = LifecycleRecommendation::from_stats(&stats, std::time::SystemTime::now());

        Ok(RouteResult { capability: capability.to_string(), skill: selected, execution, lifecycle })
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
            _ => {
                match payload.autonomous.recovery_strategy {
                    aion_types::ai_native::RecoveryStrategy::ReSynthesize => {
                        warn!("Primary execution failed — autonomous recovery: re-synthesizing skill for '{}'", payload.intent);
                        let discovery = aion_intel::discovery_radar::DiscoveryRadar::cascade_search(&payload.intent, &self.paths).await?;
                        let discovery_json = aion_intel::discovery_radar::DiscoveryRadar::to_json(&discovery);

                        let capability = payload.capability.clone().unwrap_or_else(|| payload.intent.clone());
                        let _recovered_skill = {
                            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("lock: {}", e))?;
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
                        warn!("Primary execution failed — autonomous recovery: falling back to capability '{}'", cap);
                        self.route_inner(&payload.intent, cap, Some(payload.parameters.clone())).await
                    }
                    aion_types::ai_native::RecoveryStrategy::None => primary_result,
                    aion_types::ai_native::RecoveryStrategy::AgentFailover { ref preferred, ref fallback_agents } => {
                        let fallback_cap = fallback_agents.first()
                            .map(|s| s.as_str())
                            .unwrap_or(preferred.as_str());
                        warn!("AgentFailover: distributed mode not yet available, falling back to capability '{}'", fallback_cap);
                        self.route_inner(&payload.intent, fallback_cap, Some(payload.parameters.clone())).await
                    }
                }
            }
        }
    }
}
