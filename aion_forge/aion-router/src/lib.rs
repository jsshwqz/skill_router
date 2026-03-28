pub mod executor;
pub mod registry;
pub mod loader;
pub mod matcher;
pub mod security;
pub mod parallel_executor;
pub mod coordinator;
pub mod automation;

#[cfg(test)]
mod tests;

use anyhow::Result;
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

    pub fn route(&self, task: &str) -> Result<RouteResult> {
        self.route_with_context(task, None)
    }

    pub fn route_with_context(&self, task: &str, context: Option<serde_json::Value>) -> Result<RouteResult> {
        let capability = {
            let mut reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            Planner::infer_capability_with_paths(task, &mut reg, &self.paths)?
                .ok_or_else(|| anyhow::anyhow!("could not infer capability for task: '{task}'"))?
        };
        self.route_inner(task, &capability, context)
    }

    pub fn route_with_capability(&self, task: &str, capability: &str, context: Option<serde_json::Value>) -> Result<RouteResult> {
        {
            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            reg.validate_name(capability)
                .map_err(|_| anyhow::anyhow!("unknown capability: {capability}"))?;
        }
        self.route_inner(task, capability, context)
    }

    fn route_inner(&self, task: &str, capability: &str, extra_context: Option<serde_json::Value>) -> Result<RouteResult> {
        let selected = {
            let reg = self.capability_registry.lock().map_err(|e| anyhow::anyhow!("registry lock poisoned: {}", e))?;
            let local_skills = Loader::load_local_skills(&self.paths, &reg)?;
            let matching_local: Vec<SkillDefinition> = local_skills
                .into_iter()
                .filter(|skill| skill.supports_capability(capability))
                .collect();
            
            let trusted = TrustedSourceSearch::search(&self.paths, capability).unwrap_or_default();
            let registry_store = RegistryStore::load(&self.paths)?;

            if !matching_local.is_empty() {
                // Tier 1: Local Match
                Matcher::select_best_with_registry(capability, &matching_local, &trusted, Some(&registry_store))?
            } else {
                // Tier 2: Cascade Discovery (New Evolution Feature)
                println!("🔍 Local miss. Triggering DiscoveryRadar for cascade search...");
                let discovery = aion_intel::discovery_radar::DiscoveryRadar::cascade_search(task, &self.paths)?;
                
                // Tier 3: Synthesis with enhanced context from discovery
                println!("🧬 Synthesizing new skill with discovery context...");
                let discovery_json = aion_intel::discovery_radar::DiscoveryRadar::to_json(&discovery);
                let synthesized = Synthesizer::create_placeholder_with_context(
                    &self.paths, 
                    capability, 
                    task,
                    Some(discovery_json) // Use real search results to guide synthesis
                )?;
                
                Matcher::select_best_with_registry(capability, &[synthesized], &trusted, Some(&registry_store))?
            }
        };

        let exec_ctx = {
            let mut ctx = ExecutionContext::new(task, capability);
            if let Some(extra) = extra_context { ctx = ctx.with_context(extra); }
            ctx
        };

        let execution = Executor::execute(&selected, &exec_ctx, &self.paths)?;

        let mut registry = RegistryStore::load(&self.paths)?;
        registry.record_execution(&selected.metadata.name, execution.status == "ok", std::time::SystemTime::now());
        registry.save(&self.paths)?;

        let stats = registry.skill_stats(&selected.metadata.name)
            .ok_or_else(|| anyhow::anyhow!("missing registry stats for {}", selected.metadata.name))?;
        let lifecycle = LifecycleRecommendation::from_stats(&stats, std::time::SystemTime::now());

        Ok(RouteResult { capability: capability.to_string(), skill: selected, execution, lifecycle })
    }

    /// AI-native entry-point: accept a structured payload instead of natural language.
    pub fn route_native(&self, payload: AiNativePayload) -> Result<RouteResult> {
        let route_fn = |p: &AiNativePayload| {
            let ctx = p.to_execution_context();
            match &p.capability {
                Some(cap) => self.route_inner(&ctx.task, cap, Some(ctx.context.clone())),
                None => self.route_with_context(&ctx.task, Some(ctx.context.clone())),
            }
        };

        let primary_result = route_fn(&payload);

        match primary_result {
            Ok(res) if res.execution.status == "ok" => Ok(res),
            _ => {
                match payload.autonomous.recovery_strategy {
                    aion_types::ai_native::RecoveryStrategy::ReSynthesize => {
                        println!("🛡️  Autonomous Recovery: primary execution failed. Starting Re-Synthesis...");
                        // Strategy: Use DiscoveryRadar to get fresh context and override synthesis
                        let discovery = aion_intel::discovery_radar::DiscoveryRadar::cascade_search(&payload.intent, &self.paths)?;
                        let discovery_json = aion_intel::discovery_radar::DiscoveryRadar::to_json(&discovery);
                        
                        let capability = payload.capability.clone().unwrap_or_else(|| payload.intent.clone());
                        let _recovered_skill = Synthesizer::create_placeholder_with_context(
                            &self.paths, 
                            &capability, 
                            &payload.intent, 
                            Some(discovery_json)
                        )?;
                        
                        // Retry execution with the same payload (now that skill is re-synthesized)
                        println!("🔄 Retrying with re-synthesized skill...");
                        route_fn(&payload)
                    }
                    aion_types::ai_native::RecoveryStrategy::Fallback(ref cap) => {
                        println!("🛡️  Autonomous Recovery: falling back to capability '{}'", cap);
                        self.route_inner(&payload.intent, cap, Some(payload.parameters.clone()))
                    }
                    aion_types::ai_native::RecoveryStrategy::None => primary_result,
                }
            }
        }
    }
}
