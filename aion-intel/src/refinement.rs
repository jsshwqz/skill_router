//! 自我改进引擎
//!
//! 分析执行历史，发现能力缺口，建议改进方向。

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 能力缺口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGap {
    /// 失败的任务模式（关键词）
    pub task_pattern: String,
    /// 出现频率
    pub frequency: usize,
    /// 建议的能力名称
    pub suggested_capability: String,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f64,
}

/// 改进类别
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementCategory {
    /// 新增能力
    NewCapability,
    /// 现有能力优化
    PerformanceOptimization,
    /// 错误修复
    ErrorPatternFix,
    /// 覆盖范围扩展
    CoverageExpansion,
}

/// 改进建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    /// 改进类别
    pub category: ImprovementCategory,
    /// 描述
    pub description: String,
    /// 是否可以自动应用
    pub auto_applicable: bool,
    /// 相关能力名称
    pub related_capability: Option<String>,
}

/// 自我改进引擎
pub struct RefinementEngine;

impl RefinementEngine {
    /// 分析执行日志，发现能力缺口
    pub fn detect_gaps(state_dir: &Path) -> Result<Vec<CapabilityGap>> {
        let log_path = state_dir.join("executions.log");
        if !log_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&log_path)?;
        let mut failure_patterns: HashMap<String, usize> = HashMap::new();

        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                let status = entry["status"].as_str().unwrap_or("");
                if status == "error" || status == "not_found" {
                    let capability = entry["capability"].as_str().unwrap_or("unknown");
                    *failure_patterns.entry(capability.to_string()).or_insert(0) += 1;
                }
            }
        }

        let mut gaps: Vec<CapabilityGap> = failure_patterns
            .into_iter()
            .filter(|(_, count)| *count >= 2) // 至少失败 2 次才认为是真正的缺口
            .map(|(pattern, frequency)| {
                let confidence = (frequency as f64 / 10.0).min(1.0);
                CapabilityGap {
                    task_pattern: pattern.clone(),
                    frequency,
                    suggested_capability: pattern,
                    confidence,
                }
            })
            .collect();

        gaps.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        Ok(gaps)
    }

    /// 基于执行统计建议改进
    pub fn suggest_improvements(state_dir: &Path) -> Result<Vec<Improvement>> {
        let mut improvements = Vec::new();

        // 检测能力缺口
        let gaps = Self::detect_gaps(state_dir)?;
        for gap in &gaps {
            improvements.push(Improvement {
                category: ImprovementCategory::NewCapability,
                description: format!(
                    "能力 '{}' 被请求 {} 次但执行失败，建议新增或修复此能力",
                    gap.suggested_capability, gap.frequency
                ),
                auto_applicable: false,
                related_capability: Some(gap.suggested_capability.clone()),
            });
        }

        // 检查审计日志中的超时
        let audit_path = state_dir.join("sandbox_audit.log");
        if audit_path.exists() {
            let content = std::fs::read_to_string(&audit_path)?;
            let timeout_count = content.lines()
                .filter(|line| line.contains("\"outcome\":\"timeout\""))
                .count();
            if timeout_count >= 3 {
                improvements.push(Improvement {
                    category: ImprovementCategory::PerformanceOptimization,
                    description: format!(
                        "沙箱执行出现 {} 次超时，建议增加超时限制或优化命令参数",
                        timeout_count
                    ),
                    auto_applicable: false,
                    related_capability: None,
                });
            }
        }

        Ok(improvements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_gaps_empty() {
        let tmp = std::env::temp_dir().join("aion-refinement-test-empty");
        let _ = std::fs::create_dir_all(&tmp);
        let gaps = RefinementEngine::detect_gaps(&tmp).unwrap();
        assert!(gaps.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_gaps_with_failures() {
        let tmp = std::env::temp_dir().join("aion-refinement-test-gaps");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let log = r#"{"capability":"video_edit","status":"error","skill":"placeholder"}
{"capability":"video_edit","status":"error","skill":"placeholder"}
{"capability":"video_edit","status":"error","skill":"placeholder"}
{"capability":"text_summarize","status":"ok","skill":"builtin"}
{"capability":"image_gen","status":"error","skill":"placeholder"}
"#;
        std::fs::write(tmp.join("executions.log"), log).unwrap();

        let gaps = RefinementEngine::detect_gaps(&tmp).unwrap();
        assert_eq!(gaps.len(), 1); // only video_edit has >= 2 failures
        assert_eq!(gaps[0].suggested_capability, "video_edit");
        assert_eq!(gaps[0].frequency, 3);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_suggest_improvements() {
        let tmp = std::env::temp_dir().join("aion-refinement-test-improve");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let log = r#"{"capability":"missing_cap","status":"error","skill":"x"}
{"capability":"missing_cap","status":"error","skill":"x"}
"#;
        std::fs::write(tmp.join("executions.log"), log).unwrap();

        let improvements = RefinementEngine::suggest_improvements(&tmp).unwrap();
        assert!(!improvements.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
