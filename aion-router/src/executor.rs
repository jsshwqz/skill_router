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
use aion_types::types::{ExecutionContext, ExecutionResponse, RouterPaths, SkillDefinition, TokenUsage};

/// 全局 builtin 注册表（进程生命周期内只初始化一次）
fn builtin_registry() -> &'static BuiltinRegistry {
    use std::sync::OnceLock;
    static REGISTRY: OnceLock<BuiltinRegistry> = OnceLock::new();
    REGISTRY.get_or_init(BuiltinRegistry::default_registry)
}

pub struct Executor;

impl Executor {
    fn execution_source(context: &ExecutionContext) -> String {
        if let Some(source) = context.context.get("source").and_then(|v| v.as_str()) {
            return source.to_string();
        }
        if std::env::var("AION_MCP_MODE").ok().as_deref() == Some("1") {
            return "mcp".to_string();
        }
        "cli".to_string()
    }

    pub fn validate_permissions(skill: &SkillDefinition, paths: &RouterPaths) -> Result<()> {
        Security::validate(skill, paths)
    }

    pub async fn execute(
        skill: &SkillDefinition,
        context: &ExecutionContext,
        paths: &RouterPaths,
    ) -> Result<ExecutionResponse> {
        Self::validate_permissions(skill, paths)?;

        // 可选前置治理（AION_EVOLVER_GOVERNANCE=true 时启用）
        if std::env::var("AION_EVOLVER_GOVERNANCE").map(|v| v == "true").unwrap_or(false) {
            if let Some(governance) = Self::run_governance(context).await? {
                return Ok(ExecutionResponse {
                    status: "governance_blocked".into(),
                    result: governance,
                    artifacts: serde_json::json!({}),
                    error: Some("task requires clarification before execution".into()),
                    token_usage: None,
                });
            }
        }

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

        // CONTROL_CHARACTER_FLOOD 防御：检查 \r 控制字符数量
        ImmunitySystem::check_control_character_flood(&sanitized_task)?;

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

        // 记录 Token 消耗（从 ExecutionResponse 的 token_usage 字段提取）
        if let Ok(ref resp) = response {
            if let Some(token_usage) = &resp.token_usage {
                let provider = resp.result.get("provider")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown");
                crate::metrics::record_token_usage(
                    &skill.metadata.name,
                    &context.capability,
                    provider,
                    token_usage,
                );
            }
        }

        // 学习引擎：持久化记录执行结果（含来源与失败分类）
        if let Some(learner) = crate::learner::learner() {
            let source = Self::execution_source(context);
            let error = response
                .as_ref()
                .err()
                .map(|e| e.to_string());
            let empty_output = response
                .as_ref()
                .ok()
                .map(|r| r.result.is_null() || r.result == Value::String(String::new()))
                .unwrap_or(false);
            learner.record_execution(
                &context.capability,
                &skill.metadata.name,
                &source,
                success,
                duration,
                error.as_deref(),
                empty_output,
            );
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
                token_usage: None,
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

        // 从 result 中提取 token_usage（ai_task builtin 会附带）
        let token_usage = result.get("token_usage")
            .and_then(|v| serde_json::from_value::<TokenUsage>(v.clone()).ok());

        Ok(ExecutionResponse {
            status: "ok".to_string(),
            result,
            artifacts: Value::Object(Default::default()),
            error: None,
            token_usage,
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
            token_usage: None,
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

// ── 前置治理（可选）─────────────────────────────────────────────────────────
impl Executor {
    /// 当 AION_EVOLVER_GOVERNANCE=true 时，对模糊/高风险任务返回治理建议
    async fn run_governance(context: &ExecutionContext) -> Result<Option<Value>> {
        use crate::builtins::orchestrator::call_http_ai_fallback;

        let task = ctx_get_task(context);
        let prompt = format!(
            "You are a governance analyst. Classify the task and output JSON only:\n\
             {{\"clarity\":\"clear|vague|ambiguous\",\"risk\":\"low|medium|high\",\
             \"critical_assumptions\":[],\"rationale\":\"\"}}\n\n<task>{}</task>",
            task
        );

        let report = call_http_ai_fallback(&prompt, "governance").await;
        let output = match &report.output {
            Some(o) => o,
            None => return Ok(None),
        };

        let parsed: Value = serde_json::from_str(output).unwrap_or_default();
        let clarity = parsed["clarity"].as_str().unwrap_or("clear");
        let risk = parsed["risk"].as_str().unwrap_or("low");

        if clarity != "clear" || risk == "high" {
            Ok(Some(json!({
                "governance": parsed,
                "recommendation": "Task needs clarification before execution",
            })))
        } else {
            Ok(None)
        }
    }
}

fn ctx_get_task(context: &ExecutionContext) -> &str {
    context.context["task"]
        .as_str()
        .or_else(|| context.context["text"].as_str())
        .unwrap_or(&context.task)
}
