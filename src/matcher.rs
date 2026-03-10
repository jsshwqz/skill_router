use crate::models::{Registry, SkillMetadata};

pub struct Matcher;

impl Matcher {
    pub fn find_skills_for_caps(registry: &Registry, required_caps: &[String]) -> Vec<SkillMetadata> {
        let mut matched_skills = Vec::new();
        for skill in registry.skills.values() {
            if required_caps.iter().any(|cap| skill.capabilities.contains(cap)) {
                matched_skills.push(skill.clone());
            }
        }
        matched_skills
    }

    pub fn find_best_match(registry: &Registry, required_caps: &[String]) -> Option<SkillMetadata> {
        let mut best_match: Option<(SkillMetadata, f64)> = None;

        for skill in registry.skills.values() {
            let matches = required_caps.iter().filter(|cap| skill.capabilities.contains(cap)).count();
            if matches == 0 {
                continue;
            }

            // Score calculation
            let mut score = matches as f64 * 10.0;

            // Bonus for non-synthesized skills (local first)
            if let Some(source) = &skill.source {
                if source != "synth_generated" {
                    score += 5.0;
                }
            } else {
                // Skills without source are assumed local
                score += 5.0;
            }

            // Consider success rate
            if let Some(usage) = &skill.usage {
                if usage.total_calls > 0 {
                    let success_rate = (usage.success_calls as f64) / (usage.total_calls as f64);
                    score += success_rate * 5.0;
                }
            }

            if let Some((_, best_score)) = &best_match {
                if score > *best_score {
                    best_match = Some((skill.clone(), score));
                }
            } else {
                best_match = Some((skill.clone(), score));
            }
        }

        best_match.map(|(skill, _)| skill)
    }
}
