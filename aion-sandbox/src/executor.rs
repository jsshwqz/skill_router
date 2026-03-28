//! 沙箱化命令执行器
//!
//! `SandboxedExecutor` 在白名单策略约束下安全地执行外部命令。
//! 所有执行受超时控制、输出截断、环境变量过滤保护。

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::audit::AuditLog;
use crate::jail::{truncate_output, ResourceLimits};
use crate::policy::SandboxPolicy;

/// 要执行的沙箱命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxedCommand {
    /// 可执行文件名（必须在策略白名单中）
    pub command: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 额外环境变量（与策略过滤后的合并）
    #[serde(default)]
    pub extra_env: std::collections::HashMap<String, String>,
    /// 工作目录覆盖（None = 使用策略默认）
    #[serde(default)]
    pub work_dir: Option<PathBuf>,
}

/// 沙箱执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxOutput {
    /// 退出码（None = 超时被 kill）
    pub exit_code: Option<i32>,
    /// stdout 内容
    pub stdout: String,
    /// stderr 内容
    pub stderr: String,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// stdout 是否被截断
    pub stdout_truncated: bool,
    /// stderr 是否被截断
    pub stderr_truncated: bool,
}

/// 沙箱化命令执行器
pub struct SandboxedExecutor {
    policy: SandboxPolicy,
    state_dir: PathBuf,
}

impl SandboxedExecutor {
    /// 创建执行器
    pub fn new(policy: SandboxPolicy, state_dir: &Path) -> Self {
        Self {
            policy,
            state_dir: state_dir.to_path_buf(),
        }
    }

    /// 获取策略引用
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// 在沙箱内执行命令
    pub async fn execute(&self, cmd: &SandboxedCommand) -> Result<SandboxOutput> {
        // 1. 检查命令是否在白名单中
        let rule = self.policy.get_rule(&cmd.command).ok_or_else(|| {
            anyhow!(
                "command '{}' not in sandbox policy whitelist '{}'",
                cmd.command,
                self.policy.name
            )
        })?;

        // 2. 验证参数
        rule.validate_args(&cmd.args)?;

        // 3. 构建资源限制
        let limits = ResourceLimits {
            timeout: rule.timeout(),
            max_output_bytes: rule.max_output_bytes,
            allowed_env_vars: rule.allowed_env_vars.clone(),
        };

        // 4. 确定工作目录
        let work_dir = match &cmd.work_dir {
            Some(dir) => dir.clone(),
            None => match &rule.work_dir_policy {
                crate::policy::WorkDirPolicy::TempDir => std::env::temp_dir(),
                crate::policy::WorkDirPolicy::Inherit => {
                    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
                }
                crate::policy::WorkDirPolicy::Specified(p) => p.clone(),
            },
        };

        // 5. 构建进程
        let mut process = tokio::process::Command::new(&cmd.command);
        process
            .args(&cmd.args)
            .current_dir(&work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // 环境变量：先清空，只透传白名单 + extra_env
        process.env_clear();
        // 基本系统路径（否则很多命令找不到依赖）
        if let Ok(path) = std::env::var("PATH") {
            process.env("PATH", path);
        }
        #[cfg(target_os = "windows")]
        if let Ok(sys_root) = std::env::var("SystemRoot") {
            process.env("SystemRoot", sys_root);
        }
        for (k, v) in limits.filtered_env() {
            process.env(&k, &v);
        }
        for (k, v) in &cmd.extra_env {
            process.env(k, v);
        }

        // 6. 执行并计时
        let start = Instant::now();
        tracing::info!(
            command = %cmd.command,
            args = ?cmd.args,
            policy = %self.policy.name,
            "sandbox: executing command"
        );

        let result = tokio::time::timeout(limits.timeout, process.output()).await;
        let duration = start.elapsed();

        let audit_log = AuditLog::new(&self.state_dir);

        match result {
            Ok(Ok(output)) => {
                let (stdout_bytes, stdout_truncated) =
                    truncate_output(&output.stdout, limits.max_output_bytes);
                let (stderr_bytes, stderr_truncated) =
                    truncate_output(&output.stderr, limits.max_output_bytes);

                let exit_code = output.status.code();
                let outcome = if output.status.success() {
                    "success"
                } else {
                    "error"
                };

                let entry = AuditLog::make_entry(
                    &self.policy.name,
                    &cmd.command,
                    &cmd.args,
                    exit_code,
                    duration,
                    stdout_truncated,
                    stderr_truncated,
                    outcome,
                );
                let _ = audit_log.record(&entry);

                Ok(SandboxOutput {
                    exit_code,
                    stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                    stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                    duration_ms: duration.as_millis() as u64,
                    stdout_truncated,
                    stderr_truncated,
                })
            }
            Ok(Err(e)) => {
                let entry = AuditLog::make_entry(
                    &self.policy.name,
                    &cmd.command,
                    &cmd.args,
                    None,
                    duration,
                    false,
                    false,
                    "spawn_error",
                );
                let _ = audit_log.record(&entry);

                Err(anyhow!("failed to spawn command '{}': {}", cmd.command, e))
            }
            Err(_) => {
                // 超时
                let entry = AuditLog::make_entry(
                    &self.policy.name,
                    &cmd.command,
                    &cmd.args,
                    None,
                    duration,
                    false,
                    false,
                    "timeout",
                );
                let _ = audit_log.record(&entry);

                Err(anyhow!(
                    "command '{}' timed out after {:?}",
                    cmd.command,
                    limits.timeout
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{CommandRule, WorkDirPolicy};
    use std::collections::BTreeMap;

    fn test_policy() -> SandboxPolicy {
        let mut commands = BTreeMap::new();
        commands.insert(
            "echo".to_string(),
            CommandRule {
                allowed_args_patterns: vec![],
                blocked_args_patterns: vec![],
                timeout_secs: 5,
                max_output_bytes: 4096,
                allowed_env_vars: vec![],
                work_dir_policy: WorkDirPolicy::TempDir,
                description: "echo for testing".to_string(),
            },
        );
        SandboxPolicy {
            name: "test".to_string(),
            version: "1.0".to_string(),
            description: "test policy".to_string(),
            allowed_commands: commands,
            max_concurrent: 4,
        }
    }

    #[tokio::test]
    async fn test_execute_allowed_command() {
        let tmp = std::env::temp_dir().join("aion-sandbox-exec-test");
        let _ = std::fs::create_dir_all(&tmp);

        let executor = SandboxedExecutor::new(test_policy(), &tmp);
        let cmd = SandboxedCommand {
            command: "echo".to_string(),
            args: vec!["hello".to_string(), "sandbox".to_string()],
            extra_env: Default::default(),
            work_dir: None,
        };

        let result = executor.execute(&cmd).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, Some(0));
        assert!(output.stdout.contains("hello sandbox"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_execute_blocked_command() {
        let tmp = std::env::temp_dir().join("aion-sandbox-blocked-test");
        let _ = std::fs::create_dir_all(&tmp);

        let executor = SandboxedExecutor::new(test_policy(), &tmp);
        let cmd = SandboxedCommand {
            command: "rm".to_string(),
            args: vec!["-rf".to_string(), "/".to_string()],
            extra_env: Default::default(),
            work_dir: None,
        };

        let result = executor.execute(&cmd).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not in sandbox policy whitelist"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let mut commands = BTreeMap::new();
        commands.insert(
            "sleep".to_string(),
            CommandRule {
                allowed_args_patterns: vec![],
                blocked_args_patterns: vec![],
                timeout_secs: 1,
                max_output_bytes: 1024,
                allowed_env_vars: vec![],
                work_dir_policy: WorkDirPolicy::TempDir,
                description: "sleep for timeout test".to_string(),
            },
        );
        let policy = SandboxPolicy {
            name: "timeout-test".to_string(),
            version: "1.0".to_string(),
            description: "test".to_string(),
            allowed_commands: commands,
            max_concurrent: 1,
        };

        let tmp = std::env::temp_dir().join("aion-sandbox-timeout-test");
        let _ = std::fs::create_dir_all(&tmp);

        let executor = SandboxedExecutor::new(policy, &tmp);
        let cmd = SandboxedCommand {
            command: "sleep".to_string(),
            args: vec!["10".to_string()],
            extra_env: Default::default(),
            work_dir: None,
        };

        let result = executor.execute(&cmd).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
