//! 技能执行器
//!
//! `Executor` 负责安全审查、免疫系统预检，然后将 builtin 技能
//! 委派给 `BuiltinRegistry` 中注册的 trait 实现。
//!
//! 原有 374 行巨型 match 已拆分到 `builtins/` 子模块，每个技能
//! 类别一个文件（解析、文本、网络、记忆、AI、Agent、管道、新技能）。

use std::{
    fs::{self, OpenOptions},
    io::Write,
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::builtins::BuiltinRegistry;
use crate::security::{AiSecurityReviewer, Security, Verdict};
use aion_intel::immunity::ImmunitySystem;
use aion_sandbox::{SandboxedCommand, SandboxedExecutor, SandboxPolicy};
use aion_types::types::{ExecutionContext, ExecutionResponse, RouterPaths, SkillDefinition};

/// 全局 builtin 注册表（进程生命周期内只初始化一次）
fn builtin_registry() -> &'static BuiltinRegistry {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();
    REGISTRY.get_or_init(BuiltinRegistry::default_registry)
}

pub struct Executor;

impl Executor {
    pub fn validate_permissions(skill: &SkillDefinition, paths: &RouterPaths) -> Result<()> {
        Security::validate(skill, paths)
    }

    pub async fn execute(
        skill: &SkillDefinition,
        context: &ExecutionContext,
        paths: &RouterPaths,
    ) -> Result<ExecutionResponse> {
        Self::validate_permissions(skill, paths)?;
        paths.ensure_base_dirs()?;

        if let Verdict::Deny(reason) =
            AiSecurityReviewer::review_pre_execution(skill, context, paths).await
        {
            return Err(anyhow!("security review blocked execution: {}", reason));
        }

        // Immunity Pre-check & Sanitization
        let mut sanitized_task = context.task.clone();
        ImmunitySystem::sanitize_instruction(&mut sanitized_task);
        ImmunitySystem::pre_check_command(&sanitized_task)?;

        let start = std::time::Instant::now();

        let response = if skill.metadata.entrypoint.starts_with("builtin:") {
            Self::execute_builtin(skill, context).await
        } else if skill.metadata.entrypoint.starts_with("sandboxed:") {
            Self::execute_sandboxed(skill, context, paths).await
        } else {
            Err(anyhow!(
                "external entrypoints are not supported. Got: {}",
                skill.metadata.entrypoint
            ))
        };

        let duration = start.elapsed();
        let success = response.is_ok();
        crate::metrics::record_skill_execution(
            &skill.metadata.name,
            &context.capability,
            success,
            duration,
        );

        // 学习引擎：持久化记录执行结果
        if let Some(learner) = crate::learner::learner() {
            learner.record(&context.capability, success, duration);
        }

        let response = response?;

        if let Verdict::Deny(reason) =
            AiSecurityReviewer::review_post_execution(skill, &response, paths).await
        {
            return Err(anyhow!("security review blocked output: {}", reason));
        }

        Self::append_log(skill, context, &response, paths)?;
        Ok(response)
    }

    /// 通过 `BuiltinRegistry` 查找并执行 builtin 技能
    async fn execute_builtin(
        skill: &SkillDefinition,
        context: &ExecutionContext,
    ) -> Result<ExecutionResponse> {
        let builtin_name = skill.metadata.entrypoint.trim_start_matches("builtin:");

        // 禁用入口
        if builtin_name == "shell_exec" {
            return Err(anyhow!("shell_exec is disabled for security reasons"));
        }

        // 占位/回退
        if builtin_name == "echo" || builtin_name == "placeholder" {
            return Ok(ExecutionResponse {
                status: "ok".to_string(),
                result: json!({
                    "task": context.task,
                    "capability": context.capability,
                    "skill": skill.metadata.name,
                    "notice": "placeholder — no real implementation for this capability yet",
                }),
                artifacts: Value::Object(Default::default()),
                error: None,
            });
        }

        // 查找注册表
        let registry = builtin_registry();
        let builtin_impl = registry.get(builtin_name).ok_or_else(|| {
            anyhow!(
                "unknown builtin: '{}' — if this is an AI-task skill, use 'builtin:ai_task' with an 'instruction' field in skill.json",
                builtin_name
            )
        })?;

        let result = builtin_impl.execute(skill, context).await?;

        Ok(ExecutionResponse {
            status: "ok".to_string(),
            result,
            artifacts: Value::Object(Default::default()),
            error: None,
        })
    }

    /// 通过沙箱执行外部命令
    async fn execute_sandboxed(
        skill: &SkillDefinition,
        context: &ExecutionContext,
        paths: &RouterPaths,
    ) -> Result<ExecutionResponse> {
        // 1. 验证 sandboxed_exec 权限
        if !skill.metadata.permissions.sandboxed_exec {
            return Err(anyhow!(
                "skill '{}' uses sandboxed: entrypoint but lacks sandboxed_exec permission",
                skill.metadata.name
            ));
        }

        // 2. 加载沙箱策略
        let policy_path = skill.root_dir.join("sandbox-policy.json");
        let policy = SandboxPolicy::load_from_file(&policy_path).map_err(|e| {
            anyhow!(
                "failed to load sandbox policy for '{}': {}",
                skill.metadata.name,
                e
            )
        })?;

        // 3. 检查策略是否已被用户批准
        let approved_path = paths.state_dir.join("approved-policies.json");
        let policy_hash = SandboxPolicy::content_hash(&policy_path).unwrap_or_default();
        Self::check_policy_approved(&approved_path, &skill.metadata.name, &policy_hash)?;

        // 4. 解析命令
        let command_name = skill
            .metadata
            .entrypoint
            .strip_prefix("sandboxed:")
            .unwrap_or(&skill.metadata.entrypoint);

        // 从 context 提取参数
        let args: Vec<String> = if let Some(args_val) = context.context.get("args") {
            if let Some(arr) = args_val.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(s) = args_val.as_str() {
                s.split_whitespace().map(String::from).collect()
            } else {
                vec![]
            }
        } else {
            // 用 task 作为默认参数
            context.task.split_whitespace().map(String::from).collect()
        };

        let cmd = SandboxedCommand {
            command: command_name.to_string(),
            args,
            extra_env: Default::default(),
            work_dir: None,
        };

        // 5. 执行
        let executor = SandboxedExecutor::new(policy, &paths.state_dir);
        let output = executor.execute(&cmd).await?;

        Ok(ExecutionResponse {
            status: if output.exit_code == Some(0) {
                "ok".to_string()
            } else {
                "error".to_string()
            },
            result: json!({
                "stdout": output.stdout,
                "stderr": output.stderr,
                "exit_code": output.exit_code,
                "duration_ms": output.duration_ms,
            }),
            artifacts: json!({
                "stdout_truncated": output.stdout_truncated,
                "stderr_truncated": output.stderr_truncated,
            }),
            error: if output.exit_code != Some(0) {
                Some(format!(
                    "command exited with code {:?}: {}",
                    output.exit_code,
                    output.stderr.chars().take(200).collect::<String>()
                ))
            } else {
                None
            },
        })
    }

    /// 检查策略是否已被用户批准
    fn check_policy_approved(
        approved_path: &std::path::Path,
        skill_name: &str,
        policy_hash: &str,
    ) -> Result<()> {
        if !approved_path.exists() {
            return Err(anyhow!(
                "sandbox policy for '{}' has not been approved. \
                 Run `aion-cli sandbox approve {}` to review and approve.",
                skill_name,
                skill_name
            ));
        }

        let content = fs::read_to_string(approved_path)?;
        let approved: Value = serde_json::from_str(&content)?;

        if let Some(hash) = approved.get(skill_name).and_then(|v| v.as_str()) {
            if hash == policy_hash {
                return Ok(());
            }
            return Err(anyhow!(
                "sandbox policy for '{}' has changed since approval (hash mismatch). \
                 Re-approve with `aion-cli sandbox approve {}`.",
                skill_name,
                skill_name
            ));
        }

        Err(anyhow!(
            "sandbox policy for '{}' has not been approved. \
             Run `aion-cli sandbox approve {}` to review and approve.",
            skill_name,
            skill_name
        ))
    }

    fn append_log(
        skill: &SkillDefinition,
        context: &ExecutionContext,
        response: &ExecutionResponse,
        paths: &RouterPaths,
    ) -> Result<()> {
        if let Some(parent) = paths.executions_log.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.executions_log)?;
        let line = json!({
            "timestamp": SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "skill": skill.metadata.name,
            "capability": context.capability,
            "status": response.status
        });
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
        Ok(())
    }
}
