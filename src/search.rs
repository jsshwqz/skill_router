use crate::models::{Registry, SkillMetadata};
use std::collections::HashMap;

/// 混合搜索模块 - 结合多种搜索策略
pub struct HybridSearch;

impl HybridSearch {
    /// 执行混合搜索，返回排序后的技能列表
    pub fn hybrid_search(
        registry: &Registry,
        query: &str,
        required_caps: &[String],
    ) -> Vec<(SkillMetadata, f64)> {
        let mut results = Vec::new();
        
        // 策略1: 基于能力的精确匹配
        let exact_matches = Self::exact_capability_match(registry, required_caps);
        results.extend(exact_matches);
        
        // 策略2: 基于关键词的语义搜索
        let semantic_matches = Self::semantic_keyword_search(registry, query);
        results.extend(semantic_matches);
        
        // 策略3: 基于使用历史的推荐
        let usage_based_matches = Self::usage_based_recommendation(registry, required_caps);
        results.extend(usage_based_matches);
        
        // 策略4: 基于描述的模糊匹配
        let fuzzy_matches = Self::fuzzy_description_match(registry, query);
        results.extend(fuzzy_matches);
        
        // 合并和去重结果
        let mut combined_results: HashMap<String, (SkillMetadata, f64)> = HashMap::new();
        for (skill, score) in results {
            let key = skill.name.clone();
            if let Some((_, existing_score)) = combined_results.get(&key) {
                if score > *existing_score {
                    combined_results.insert(key, (skill, score));
                }
            } else {
                combined_results.insert(key, (skill, score));
            }
        }
        
        // 转换为向量并排序
        let mut final_results: Vec<(SkillMetadata, f64)> = combined_results
            .into_values()
            .collect();
        
        final_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        final_results
    }
    
    /// 精确能力匹配
    fn exact_capability_match(
        registry: &Registry,
        required_caps: &[String],
    ) -> Vec<(SkillMetadata, f64)> {
        let mut results = Vec::new();
        
        for skill in registry.skills.values() {
            let matched_count = skill
                .capabilities
                .iter()
                .filter(|cap| required_caps.contains(cap))
                .count();
            
            if matched_count > 0 {
                let score = matched_count as f64 / required_caps.len() as f64;
                // 精确匹配给予高分
                let boosted_score = score * 1.5;
                results.push((skill.clone(), boosted_score));
            }
        }
        
        results
    }
    
    /// 语义关键词搜索
    fn semantic_keyword_search(
        registry: &Registry,
        query: &str,
    ) -> Vec<(SkillMetadata, f64)> {
        let query_keywords = Self::extract_keywords(query);
        let mut results = Vec::new();
        
        for skill in registry.skills.values() {
            let skill_keywords = Self::extract_skill_keywords(skill);
            let similarity = Self::calculate_jaccard_similarity(&query_keywords, &skill_keywords);
            
            if similarity > 0.0 {
                // 语义搜索分数基于相似度
                let score = similarity * 0.8;
                results.push((skill.clone(), score));
            }
        }
        
        results
    }
    
    /// 基于使用历史的推荐
    fn usage_based_recommendation(
        registry: &Registry,
        required_caps: &[String],
    ) -> Vec<(SkillMetadata, f64)> {
        let mut results = Vec::new();
        
        for skill in registry.skills.values() {
            if let Some(usage) = &skill.usage {
                if usage.total_calls > 0 {
                    let success_rate = usage.success_calls as f64 / usage.total_calls as f64;
                    let avg_latency_factor = 1.0 / (1.0 + (usage.avg_latency_ms / 1000.0));
                    
                    // 计算与所需能力的相关性
                    let capability_relevance = skill
                        .capabilities
                        .iter()
                        .filter(|cap| required_caps.contains(cap))
                        .count() as f64 / required_caps.len() as f64;
                    
                    // 综合分数：成功率 + 延迟因子 + 能力相关性
                    let score = (success_rate * 0.4 + avg_latency_factor * 0.3 + capability_relevance * 0.3) * 1.2;
                    results.push((skill.clone(), score));
                }
            }
        }
        
        results
    }
    
    /// 模糊描述匹配
    fn fuzzy_description_match(
        registry: &Registry,
        query: &str,
    ) -> Vec<(SkillMetadata, f64)> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();
        
        for skill in registry.skills.values() {
            if let Some(description) = &skill.description {
                let desc_lower = description.to_lowercase();
                
                // 计算编辑距离或简单包含匹配
                let mut score = 0.0;
                if desc_lower.contains(&query_lower) {
                    score = 0.3;
                } else {
                    // 简单的词匹配
                    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
                    let desc_words: Vec<&str> = desc_lower.split_whitespace().collect();
                    
                    let matches = query_words
                        .iter()
                        .filter(|&word| desc_words.iter().any(|desc_word| desc_word.contains(word)))
                        .count();
                    
                    if matches > 0 {
                        score = matches as f64 / query_words.len() as f64 * 0.2;
                    }
                }
                
                if score > 0.0 {
                    results.push((skill.clone(), score));
                }
            }
        }
        
        results
    }
    
    /// 提取关键词（简化版）
    fn extract_keywords(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|word| word.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }
    
    /// 提取技能关键词
    fn extract_skill_keywords(skill: &SkillMetadata) -> Vec<String> {
        let mut keywords = Vec::new();
        
        // 添加能力关键词
        for cap in &skill.capabilities {
            keywords.extend(Self::extract_keywords(cap));
        }
        
        // 添加名称关键词
        keywords.extend(Self::extract_keywords(&skill.name));
        
        // 添加描述关键词
        if let Some(desc) = &skill.description {
            keywords.extend(Self::extract_keywords(desc));
        }
        
        keywords
    }
    
    /// 计算 Jaccard 相似度
    fn calculate_jaccard_similarity(set1: &[String], set2: &[String]) -> f64 {
        if set1.is_empty() && set2.is_empty() {
            return 1.0;
        }
        
        let set1_unique: std::collections::HashSet<&String> = set1.iter().collect();
        let set2_unique: std::collections::HashSet<&String> = set2.iter().collect();
        
        let intersection: std::collections::HashSet<_> = set1_unique.intersection(&set2_unique).collect();
        let union: std::collections::HashSet<_> = set1_unique.union(&set2_unique).collect();
        
        if union.is_empty() {
            0.0
        } else {
            intersection.len() as f64 / union.len() as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Permissions, UsageStats};
    
    #[test]
    fn test_exact_capability_match() {
        let mut registry = Registry {
            skills: std::collections::HashMap::new(),
        };
        
        let skill = SkillMetadata {
            name: "yaml_parser".to_string(),
            version: "0.0.1".to_string(),
            capabilities: vec!["yaml_parse".to_string(), "json_parse".to_string()],
            source: None,
            path: None,
            permissions: Permissions::default(),
            usage: None,
            lifecycle: None,
            description: Some("Parses YAML and JSON files".to_string()),
            entrypoint: Some("main.rs".to_string()),
        };
        
        registry.skills.insert("yaml_parser".to_string(), skill);
        
        let caps = vec!["yaml_parse".to_string()];
        let matches = HybridSearch::exact_capability_match(&registry, &caps);
        
        assert_eq!(matches.len(), 1);
        assert!(matches[0].1 > 1.0); // 应该有提升分
    }
    
    #[test]
    fn test_jaccard_similarity() {
        let set1 = vec!["hello".to_string(), "world".to_string()];
        let set2 = vec!["world".to_string(), "test".to_string()];
        
        let similarity = HybridSearch::calculate_jaccard_similarity(&set1, &set2);
        assert!((similarity - 0.3333333333333333).abs() < 0.001); // 1/3
    }
    
    #[test]
    fn test_hybrid_search() {
        let mut registry = Registry {
            skills: std::collections::HashMap::new(),
        };
        
        let mut usage = UsageStats::default();
        usage.total_calls = 10;
        usage.success_calls = 9;
        usage.avg_latency_ms = 50.0;
        
        let skill = SkillMetadata {
            name: "yaml_parser".to_string(),
            version: "0.0.1".to_string(),
            capabilities: vec!["yaml_parse".to_string()],
            source: None,
            path: None,
            permissions: Permissions::default(),
            usage: Some(usage),
            lifecycle: None,
            description: Some("A great YAML parser skill".to_string()),
            entrypoint: Some("main.rs".to_string()),
        };
        
        registry.skills.insert("yaml_parser".to_string(), skill);
        
        let query = "parse yaml files";
        let required_caps = vec!["yaml_parse".to_string()];
        
        let results = HybridSearch::hybrid_search(&registry, query, &required_caps);
        
        assert!(!results.is_empty());
        assert!(results[0].1 > 0.0);
    }
}