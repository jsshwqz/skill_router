use crate::executor::Executor;
use crate::models::{Config, Registry, SkillMetadata};
use anyhow::Result;

/// SkillsFinder - 智能技能发现模块
///
/// 职责：
/// 1. 利用现有搜索技能（如 google_search）查找相关技能
/// 2. 分析技能描述和元数据进行推荐
/// 3. 对发现技能进行评分排序
pub struct SkillsFinder;

impl SkillsFinder {
    /// 主发现入口：尝试通过现有技能发现新技能
    pub fn discover_skills(
        registry: &Registry,
        config: &Config,
        required_caps: &[String],
        _task: &str,
    ) -> Option<Vec<SkillMetadata>> {
        println!("[FINDER] Starting intelligent skill discovery...");
        println!("[FINDER] Required capabilities: {:?}", required_caps);

        // 策略1：检查是否有 google_search 技能可用于网络搜索
        if let Some(google_search_skill) = registry.skills.get("google_search") {
            println!("[FINDER] Found google_search skill, initiating network discovery...");

            if let Ok(found_skills) =
                Self::search_via_google_search(config, google_search_skill, required_caps, "")
            {
                if !found_skills.is_empty() {
                    println!(
                        "[FINDER] Network discovery successful, found {} potential skills",
                        found_skills.len()
                    );
                    return Some(found_skills);
                }
            }
        }

        // 策略2：检查 registry 中的其他技能是否能提供相关能力
        let related_skills = Self::find_related_skills(registry, required_caps);
        if !related_skills.is_empty() {
            println!(
                "[FINDER] Found {} related skills in registry",
                related_skills.len()
            );
            return Some(related_skills);
        }

        println!("[FINDER] No skills found via discovery methods");
        None
    }

    /// 通过 google_search 技能进行网络发现
    fn search_via_google_search(
        config: &Config,
        google_skill: &SkillMetadata,
        required_caps: &[String],
        _task: &str,
    ) -> Result<Vec<SkillMetadata>> {
        // 构造搜索查询
        let search_query = format!(
            "skill-router {} github repository",
            required_caps.join(" OR ")
        );

        // 调用 google_search 技能
        println!(
            "[FINDER] Executing google_search with query: {}",
            search_query
        );

        // 这里我们直接调用 Executor 来执行 google_search
        // 注意：在真实场景中，我们可能需要传递搜索参数给技能
        let _result = Executor::execute(config, google_skill, true);

        // 由于 google_search 的输出需要解析，这里我们返回空 Vec
        // 实际实现中应该解析搜索结果并返回发现的技能元数据
        Ok(vec![])
    }

    /// 在现有技能中查找相关技能
    fn find_related_skills(registry: &Registry, required_caps: &[String]) -> Vec<SkillMetadata> {
        let mut related = Vec::new();

        for skill in registry.skills.values() {
            // 计算技能相关性分数
            let score = Self::calculate_relevance_score(skill, required_caps);
            if score > 0.5 {
                related.push(skill.clone());
            }
        }

        // 按相关性排序
        related.sort_by(|a, b| {
            let score_a = Self::calculate_relevance_score(a, required_caps);
            let score_b = Self::calculate_relevance_score(b, required_caps);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        related
    }

    /// 计算技能与所需能力的相关性分数
    fn calculate_relevance_score(skill: &SkillMetadata, required_caps: &[String]) -> f64 {
        let mut score = 0.0;
        let cap_count = required_caps.len() as f64;

        // 能力匹配度
        let matched = skill
            .capabilities
            .iter()
            .filter(|cap| required_caps.contains(cap))
            .count() as f64;

        score += matched / cap_count;

        // 描述关键词匹配（简化版）
        if let Some(desc) = &skill.description {
            for cap in required_caps {
                if desc.to_lowercase().contains(&cap.to_lowercase()) {
                    score += 0.1;
                }
            }
        }

        // 成功率加权
        if let Some(usage) = &skill.usage {
            if usage.total_calls > 0 {
                let success_rate = usage.success_calls as f64 / usage.total_calls as f64;
                score *= success_rate;
            }
        }

        score.min(1.0)
    }

    /// 评分并排序技能候选项
    pub fn score_and_sort_candidates(
        candidates: &[SkillMetadata],
        required_caps: &[String],
    ) -> Vec<(SkillMetadata, f64)> {
        let mut scored: Vec<(SkillMetadata, f64)> = candidates
            .iter()
            .map(|skill| {
                let score = Self::calculate_relevance_score(skill, required_caps);
                (skill.clone(), score)
            })
            .filter(|(_, score)| *score > 0.3)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Permissions;

    #[test]
    fn test_relevance_score_calculation() {
        let skill = SkillMetadata {
            name: "test_skill".to_string(),
            version: "0.0.1".to_string(),
            capabilities: vec!["yaml_parse".to_string(), "json_parse".to_string()],
            source: None,
            permissions: Permissions::default(),
            usage: None,
            lifecycle: None,
            description: Some("A skill for parsing YAML and JSON files".to_string()),
            entrypoint: Some("main.rs".to_string()),
        };

        let caps = vec!["yaml_parse".to_string(), "web_search".to_string()];
        let score = SkillsFinder::calculate_relevance_score(&skill, &caps);

        // 应该匹配一半能力 (yaml_parse)
        assert!(score > 0.3 && score < 0.6);
    }

    #[test]
    fn test_find_related_skills() {
        let mut registry = Registry {
            skills: std::collections::HashMap::new(),
        };

        let skill1 = SkillMetadata {
            name: "yaml_parser".to_string(),
            version: "0.0.1".to_string(),
            capabilities: vec!["yaml_parse".to_string()],
            source: None,
            permissions: Permissions::default(),
            usage: None,
            lifecycle: None,
            description: Some("Parses YAML files".to_string()),
            entrypoint: Some("main.rs".to_string()),
        };

        registry.skills.insert("yaml_parser".to_string(), skill1);

        let caps = vec!["yaml_parse".to_string()];
        let related = SkillsFinder::find_related_skills(&registry, &caps);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "yaml_parser");
    }
}
