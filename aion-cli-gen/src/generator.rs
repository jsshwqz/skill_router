//! 技能包生成器
//!
//! 从 ToolAnalysis 生成 SKILL.md、sandbox-policy.json、skill.json。

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use aion_sandbox::policy::{CommandRule, SandboxPolicy, WorkDirPolicy};

use crate::analyzer::ToolAnalysis;

/// 技能包生成器
pub struct SkillGenerator;

impl SkillGenerator {
    /// 从工具分析结果生成 SKILL.md 内容
    pub fn generate_skill_md(analysis: &ToolAnalysis) -> String {
        let mut md = String::new();

        // YAML frontmatter
        md.push_str("---\n");
        md.push_str(&format!("name: {}\n", analysis.name));
        if let Some(ref v) = analysis.version {
            md.push_str(&format!("version: {}\n", v));
        }
        md.push_str(&format!("description: {}\n", analysis.description));

        // capabilities
        let caps: Vec<String> = analysis
            .subcommands
            .iter()
            .map(|sc| format!("{}_{}", analysis.name, sc.name))
            .collect();
        if !caps.is_empty() {
            md.push_str(&format!("capabilities: [{}]\n", caps.join(", ")));
        }
        md.push_str("platform: any\n");
        md.push_str("---\n\n");

        // Documentation
        md.push_str(&format!("# {}\n\n", analysis.name));
        md.push_str(&format!("{}\n\n", analysis.description));

        if !analysis.subcommands.is_empty() {
            md.push_str("## Commands\n\n");
            for sc in &analysis.subcommands {
                md.push_str(&format!("### `{}`\n\n{}\n\n", sc.name, sc.description));
            }
        }

        if !analysis.global_options.is_empty() {
            md.push_str("## Options\n\n");
            for opt in &analysis.global_options {
                let flag = opt
                    .long
                    .as_deref()
                    .or(opt.short.as_deref())
                    .unwrap_or("?");
                md.push_str(&format!("- `{}`: {}\n", flag, opt.description));
            }
        }

        md
    }

    /// 生成 sandbox-policy.json
    pub fn generate_sandbox_policy(analysis: &ToolAnalysis) -> SandboxPolicy {
        let mut commands = BTreeMap::new();

        // 主命令
        commands.insert(
            analysis.name.clone(),
            CommandRule {
                allowed_args_patterns: vec![],
                blocked_args_patterns: vec![
                    // 通用危险操作模式
                    r"rm\s+-rf".to_string(),
                    r"--delete-all".to_string(),
                    r"--force-delete".to_string(),
                ],
                timeout_secs: 30,
                max_output_bytes: 1_048_576,
                allowed_env_vars: vec!["HOME".to_string(), "USER".to_string()],
                work_dir_policy: WorkDirPolicy::TempDir,
                description: format!("Sandboxed execution of {}", analysis.name),
            },
        );

        SandboxPolicy {
            name: format!("{}-sandbox", analysis.name),
            version: "1.0".to_string(),
            description: format!(
                "Auto-generated sandbox policy for {}",
                analysis.name
            ),
            allowed_commands: commands,
            max_concurrent: 4,
        }
    }

    /// 生成 skill.json
    pub fn generate_skill_json(analysis: &ToolAnalysis) -> serde_json::Value {
        let capabilities: Vec<String> = analysis
            .subcommands
            .iter()
            .map(|sc| format!("{}_{}", analysis.name, sc.name))
            .collect();

        json!({
            "name": format!("{}-wrapper", analysis.name),
            "version": analysis.version.as_deref().unwrap_or("0.1.0"),
            "capabilities": capabilities,
            "entrypoint": format!("sandboxed:{}", analysis.name),
            "permissions": {
                "network": false,
                "filesystem_read": true,
                "filesystem_write": false,
                "process_exec": false,
                "sandboxed_exec": true
            }
        })
    }

    /// 将所有生成物写入目标目录
    pub fn write_to_dir(analysis: &ToolAnalysis, output_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(output_dir)?;

        // SKILL.md
        let skill_md = Self::generate_skill_md(analysis);
        std::fs::write(output_dir.join("SKILL.md"), skill_md)?;

        // sandbox-policy.json
        let policy = Self::generate_sandbox_policy(analysis);
        let policy_json = serde_json::to_string_pretty(&policy)?;
        std::fs::write(output_dir.join("sandbox-policy.json"), policy_json)?;

        // skill.json
        let skill_json = Self::generate_skill_json(analysis);
        let skill_str = serde_json::to_string_pretty(&skill_json)?;
        std::fs::write(output_dir.join("skill.json"), skill_str)?;

        tracing::info!(
            tool = %analysis.name,
            output_dir = %output_dir.display(),
            "skill package generated"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{OptionInfo, SubcommandInfo, ToolAnalysis};

    fn sample_analysis() -> ToolAnalysis {
        ToolAnalysis {
            name: "mytool".to_string(),
            version: Some("1.0.0".to_string()),
            description: "A test tool".to_string(),
            subcommands: vec![
                SubcommandInfo {
                    name: "build".to_string(),
                    description: "Build the project".to_string(),
                    options: vec![],
                },
                SubcommandInfo {
                    name: "test".to_string(),
                    description: "Run tests".to_string(),
                    options: vec![],
                },
            ],
            global_options: vec![OptionInfo {
                long: Some("--verbose".to_string()),
                short: Some("-v".to_string()),
                description: "Verbose output".to_string(),
                takes_value: false,
                required: false,
            }],
            raw_help: String::new(),
        }
    }

    #[test]
    fn test_generate_skill_md() {
        let md = SkillGenerator::generate_skill_md(&sample_analysis());
        assert!(md.contains("name: mytool"));
        assert!(md.contains("version: 1.0.0"));
        assert!(md.contains("# mytool"));
        assert!(md.contains("### `build`"));
    }

    #[test]
    fn test_generate_sandbox_policy() {
        let policy = SkillGenerator::generate_sandbox_policy(&sample_analysis());
        assert_eq!(policy.name, "mytool-sandbox");
        assert!(policy.allowed_commands.contains_key("mytool"));
    }

    #[test]
    fn test_generate_skill_json() {
        let json = SkillGenerator::generate_skill_json(&sample_analysis());
        assert_eq!(json["entrypoint"], "sandboxed:mytool");
        assert_eq!(json["permissions"]["sandboxed_exec"], true);
        assert_eq!(json["permissions"]["process_exec"], false);
    }

    #[test]
    fn test_write_to_dir() {
        let tmp = std::env::temp_dir().join("aion-cli-gen-test");
        let _ = std::fs::remove_dir_all(&tmp);

        SkillGenerator::write_to_dir(&sample_analysis(), &tmp).unwrap();

        assert!(tmp.join("SKILL.md").exists());
        assert!(tmp.join("sandbox-policy.json").exists());
        assert!(tmp.join("skill.json").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
