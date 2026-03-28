//! CLI 包装器生成管道
//!
//! 5 阶段管道：分析 → 生成 → 测试 → 安全审查 → 发布

use std::path::{Path, PathBuf};

use anyhow::Result;

use aion_sandbox::{SandboxPolicy, SandboxedExecutor};

use crate::analyzer::ToolAnalyzer;
use crate::generator::SkillGenerator;

/// 管道执行结果
#[derive(Debug)]
pub struct PipelineResult {
    /// 生成的技能包目录
    pub output_dir: PathBuf,
    /// 工具名称
    pub tool_name: String,
    /// 工具版本
    pub tool_version: Option<String>,
    /// 发现的子命令数
    pub subcommand_count: usize,
    /// 发现的选项数
    pub option_count: usize,
}

/// CLI 包装器生成管道
pub struct GenerationPipeline;

impl GenerationPipeline {
    /// 执行完整管道
    ///
    /// 1. 分析：运行 tool --help 提取结构
    /// 2. 生成：产出 SKILL.md + sandbox-policy.json + skill.json
    /// 3. 测试：验证生成物合法性
    /// 4. 安全审查：检验沙箱策略
    /// 5. 发布：写入目标目录
    pub async fn run(
        tool_name: &str,
        output_dir: &Path,
        sandbox_executor: &SandboxedExecutor,
    ) -> Result<PipelineResult> {
        tracing::info!(tool = %tool_name, "pipeline: starting analysis");

        // Phase 1: Analyze
        let analysis = ToolAnalyzer::analyze(tool_name, sandbox_executor).await?;
        tracing::info!(
            tool = %tool_name,
            subcommands = analysis.subcommands.len(),
            options = analysis.global_options.len(),
            "pipeline: analysis complete"
        );

        // Phase 2: Generate
        let skill_dir = output_dir.join(format!("{}-wrapper", tool_name));
        SkillGenerator::write_to_dir(&analysis, &skill_dir)?;
        tracing::info!(tool = %tool_name, "pipeline: generation complete");

        // Phase 3: Test (validate generated artifacts)
        Self::validate_artifacts(&skill_dir)?;
        tracing::info!(tool = %tool_name, "pipeline: validation passed");

        // Phase 4: Security review
        let policy_path = skill_dir.join("sandbox-policy.json");
        let policy = SandboxPolicy::load_from_file(&policy_path)?;
        Self::security_review(&policy)?;
        tracing::info!(tool = %tool_name, "pipeline: security review passed");

        // Phase 5: Done (artifacts already written)
        tracing::info!(
            tool = %tool_name,
            output = %skill_dir.display(),
            "pipeline: complete"
        );

        Ok(PipelineResult {
            output_dir: skill_dir,
            tool_name: analysis.name,
            tool_version: analysis.version,
            subcommand_count: analysis.subcommands.len(),
            option_count: analysis.global_options.len(),
        })
    }

    /// 验证生成物完整性
    fn validate_artifacts(skill_dir: &Path) -> Result<()> {
        let required_files = ["SKILL.md", "sandbox-policy.json", "skill.json"];
        for file in &required_files {
            let path = skill_dir.join(file);
            if !path.exists() {
                return Err(anyhow::anyhow!("missing required file: {}", file));
            }
            let content = std::fs::read_to_string(&path)?;
            if content.is_empty() {
                return Err(anyhow::anyhow!("file is empty: {}", file));
            }
        }

        // 验证 skill.json 是合法 JSON
        let skill_json = std::fs::read_to_string(skill_dir.join("skill.json"))?;
        let _: serde_json::Value = serde_json::from_str(&skill_json)
            .map_err(|e| anyhow::anyhow!("invalid skill.json: {}", e))?;

        // 验证 sandbox-policy.json 可解析
        let _policy = SandboxPolicy::load_from_file(&skill_dir.join("sandbox-policy.json"))?;

        Ok(())
    }

    /// 安全审查生成的策略
    fn security_review(policy: &SandboxPolicy) -> Result<()> {
        // 检查是否有明显不安全的命令
        let dangerous_commands = [
            "rm", "del", "format", "mkfs", "dd", "chmod", "chown",
            "shutdown", "reboot", "kill", "pkill", "killall",
        ];

        for cmd in policy.allowed_commands.keys() {
            let cmd_lower = cmd.to_ascii_lowercase();
            for danger in &dangerous_commands {
                if cmd_lower == *danger {
                    return Err(anyhow::anyhow!(
                        "security review failed: dangerous command '{}' in whitelist",
                        cmd
                    ));
                }
            }
        }

        // 检查超时不要太长
        for (cmd, rule) in &policy.allowed_commands {
            if rule.timeout_secs > 300 {
                tracing::warn!(
                    command = %cmd,
                    timeout = rule.timeout_secs,
                    "security review: command has very long timeout (>5min)"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use aion_sandbox::policy::CommandRule;

    #[test]
    fn test_security_review_blocks_dangerous() {
        let mut commands = BTreeMap::new();
        commands.insert("rm".to_string(), CommandRule {
            allowed_args_patterns: vec![],
            blocked_args_patterns: vec![],
            timeout_secs: 10,
            max_output_bytes: 1024,
            allowed_env_vars: vec![],
            work_dir_policy: aion_sandbox::WorkDirPolicy::TempDir,
            description: String::new(),
        });
        let policy = SandboxPolicy {
            name: "bad".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            allowed_commands: commands,
            max_concurrent: 1,
        };
        assert!(GenerationPipeline::security_review(&policy).is_err());
    }

    #[test]
    fn test_security_review_allows_safe() {
        let mut commands = BTreeMap::new();
        commands.insert("curl".to_string(), CommandRule {
            allowed_args_patterns: vec![],
            blocked_args_patterns: vec![],
            timeout_secs: 30,
            max_output_bytes: 1024,
            allowed_env_vars: vec![],
            work_dir_policy: aion_sandbox::WorkDirPolicy::TempDir,
            description: String::new(),
        });
        let policy = SandboxPolicy {
            name: "safe".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            allowed_commands: commands,
            max_concurrent: 1,
        };
        assert!(GenerationPipeline::security_review(&policy).is_ok());
    }
}
