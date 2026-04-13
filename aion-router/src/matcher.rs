use anyhow::{anyhow, Result};

use aion_types::types::{SkillDefinition, SkillSource};
use super::learner::SkillLearner;
use super::registry::RegistryStore;

pub struct Matcher;

impl Matcher {
    /// Select the best skill from candidates.
    /// Scoring: source priority (local > generated > remote) + registry success rate + usage count.
    pub fn select_best(
        capability: &str,
        primary_candidates: &[SkillDefinition],
        fallback_candidates: &[SkillDefinition],
    ) -> Result<SkillDefinition> {
        Self::select_best_with_registry(capability, primary_candidates, fallback_candidates, None)
    }

    pub fn select_best_with_registry(
        capability: &str,
        primary_candidates: &[SkillDefinition],
        fallback_candidates: &[SkillDefinition],
        registry: Option<&RegistryStore>,
    ) -> Result<SkillDefinition> {
        Self::select_best_full(capability, primary_candidates, fallback_candidates, registry, None)
    }

    /// Select the best skill using both registry stats and learner quality scores.
    /// When a learner is provided, its quality_score (which includes latency penalty,
    /// feedback bonus, and circuit breaker penalty) is used instead of raw success rate.
    pub fn select_best_full(
        capability: &str,
        primary_candidates: &[SkillDefinition],
        fallback_candidates: &[SkillDefinition],
        registry: Option<&RegistryStore>,
        learner: Option<&SkillLearner>,
    ) -> Result<SkillDefinition> {
        let mut scored: Vec<(f64, SkillDefinition)> = primary_candidates
            .iter()
            .chain(fallback_candidates.iter())
            .filter(|skill| skill.supports_capability(capability))
            .map(|skill| {
                let score = Self::score(skill, registry, learner);
                (score, skill.clone())
            })
            .collect();

        // Higher score = better; stable sort so ties keep original order
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .next()
            .map(|(_, skill)| skill)
            .ok_or_else(|| anyhow!("no candidate available for capability {capability}"))
    }

    /// Score a skill. Range roughly 0.0–10.0.
    /// - Source priority contributes 0–4 points (local=4, generated=2, remote=1)
    /// - Success rate contributes 0–3 points (uses learner quality_score if available)
    /// - Usage (30d) contributes 0–3 points (log-scaled, caps at 50 uses)
    fn score(skill: &SkillDefinition, registry: Option<&RegistryStore>, learner: Option<&SkillLearner>) -> f64 {
        let source_score = match skill.source {
            SkillSource::Local           => 4.0,
            SkillSource::ExternalCli     => 3.0,
            SkillSource::Generated       => 2.0,
            SkillSource::RemoteCandidate => 1.0,
        };

        // Try learner's quality_score first (includes latency penalty, feedback, circuit breaker).
        // Fall back to registry's raw success_rate if learner is unavailable.
        let learner_quality = learner.and_then(|l| {
            // Look up by capability name (learner tracks by capability)
            skill.metadata.capabilities.first()
                .and_then(|cap| l.get_stats(cap))
                .map(|stats| stats.quality_score())
        });

        let (success_score, usage_score) = if let Some(quality) = learner_quality {
            // Use learner quality_score for the success component
            let s = quality * 3.0;
            // Still use registry for usage frequency (learner doesn't track 30d windows)
            let u = if let Some(reg) = registry {
                if let Some(stats) = reg.skill_stats(&skill.metadata.name) {
                    let raw = (stats.uses_30d as f64 + 1.0).ln() / (50.0_f64 + 1.0).ln() * 3.0;
                    raw.min(3.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            (s, u)
        } else if let Some(reg) = registry {
            if let Some(stats) = reg.skill_stats(&skill.metadata.name) {
                let s = stats.success_rate * 3.0;
                let u = (stats.uses_30d as f64 + 1.0).ln() / (50.0_f64 + 1.0).ln() * 3.0;
                (s, u.min(3.0))
            } else {
                (1.5, 0.0)
            }
        } else {
            (1.5, 0.0)
        };

        source_score + success_score + usage_score
    }
}
