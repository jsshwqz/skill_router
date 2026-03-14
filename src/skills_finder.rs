use crate::executor::Executor;
use crate::models::{Config, Registry, SkillMetadata};
use crate::search::HybridSearch;
use anyhow::Result;

/// SkillsFinder - 智能技能发现模块
///
/// 职责：
/// 1. 利用现有搜索技能（如 google_search）查找相关技能
/// 2. 分析技能描述和元数据进行推荐
/// 3. 对发现技能进行评分排序
/// 4. 集成混合搜索策略（MemOS inspired）
pub struct SkillsFinder;

impl SkillsFinder {
    /// 主发现入口：尝试通过现有技能发现新技能
    pub fn discover_skills(
        registry: &Registry,
        config: &Config,
        required_caps: &[String],
        task: &str,
    ) -> Option<Vec<SkillMetadata>> {
        println!("[FINDER] Starting intelligent skill discovery...");
        println!("[FINDER] Required capabilities: {:?}", required_caps);
        println!("[FINDER] Task context: {}", task);

        // 策略0：首先使用混合搜索在现有注册表中查找
        let hybrid_results = HybridSearch::hybrid_search(registry, task, required_caps);
        if !hybrid_results.is_empty() {
            let top_candidates: Vec<SkillMetadata> = hybrid_results
                .into_iter()
                .take(5) // 取前5个最佳匹配
                .map(|(skill, _score)| skill)
                .collect();
            
            if !top_candidates.is_empty() {
                println!("[FINDER] Hybrid search found {} candidate skills", top_candidates.len());
                return Some(top_candidates);
            }
        }

        // 策略1：使用混合搜索策略进行网络发现
        println!("[FINDER] Initiating hybrid search discovery...");
        if let Ok(found_skills) =
            Self::search_via_hybrid_search(config, registry, required_caps, task)
        {
            if !found_skills.is_empty() {
                println!(
                    "[FINDER] Hybrid discovery successful, found {} potential skills",
                    found_skills.len()
                );
                return Some(found_skills);
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

    /// 通过混合搜索策略进行网络发现
    fn search_via_hybrid_search(
        config: &Config,
        registry: &Registry,
        required_caps: &[String],
        task: &str,
    ) -> Result<Vec<SkillMetadata>> {
        println!("[FINDER] Initiating hybrid search discovery...");
        
        // 策略1: 检查是否需要网页内容提取
        if task.contains("http") || task.contains("https") || task.contains("www.") {
            if registry.skills.contains_key("jina_reader") {
                println!("[FINDER] Using jina_reader for URL content extraction");
                // 执行 jina_reader 获取内容，然后基于内容进行技能匹配
                // 这里简化处理，直接返回相关技能
                return Ok(Self::find_related_skills(registry, required_caps));
            }
        }
        
        // 策略2: 检查是否有 google_search 技能（保留兼容性）
        if let Some(google_skill) = registry.skills.get("google_search") {
            println!("[FINDER] Fallback to google_search for general queries");
            let _result = Executor::execute(config, google_skill, true);
            // 返回相关技能
            return Ok(Self::find_related_skills(registry, required_caps));
        }

        // 策略3: 直接返回相关技能
        Ok(Self::find_related_skills(registry, required_caps))
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
            path: None,
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
            path: None,
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