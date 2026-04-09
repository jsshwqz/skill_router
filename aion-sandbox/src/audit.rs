//! 沙箱执行审计日志
//!
//! 每次沙箱化命令执行都会记录到审计日志文件，
//! 包含命令、参数、退出码、耗时等信息。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// 单条审计记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// 时间戳（Unix 秒）
    pub timestamp: u64,
    /// 应用的策略名称
    pub policy_name: String,
    /// 执行的命令
    pub command: String,
    /// 命令参数
    pub args: Vec<String>,
    /// 退出码（None = 超时或信号终止）
    pub exit_code: Option<i32>,
    /// 执行耗时
    pub duration_ms: u64,
    /// stdout 是否被截断
    pub stdout_truncated: bool,
    /// stderr 是否被截断
    pub stderr_truncated: bool,
    /// 执行结果（success / timeout / error）
    pub outcome: String,
}

/// 审计日志管理器
pub struct AuditLog {
    log_path: PathBuf,
}

impl AuditLog {
    /// 创建审计日志管理器
    pub fn new(state_dir: &Path) -> Self {
        Self {
            log_path: state_dir.join("sandbox_audit.log"),
        }
    }

    /// 记录一条审计条目
    pub fn record(&self, entry: &AuditEntry) -> anyhow::Result<()> {
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        let line = serde_json::to_string(entry)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    /// 创建一条审计记录（便捷构造方法）
    #[allow(clippy::too_many_arguments)]
    pub fn make_entry(
        policy_name: &str,
        command: &str,
        args: &[String],
        exit_code: Option<i32>,
        duration: Duration,
        stdout_truncated: bool,
        stderr_truncated: bool,
        outcome: &str,
    ) -> AuditEntry {
        AuditEntry {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            policy_name: policy_name.to_string(),
            command: command.to_string(),
            args: args.to_vec(),
            exit_code,
            duration_ms: duration.as_millis() as u64,
            stdout_truncated,
            stderr_truncated,
            outcome: outcome.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditLog::make_entry(
            "test-policy",
            "echo",
            &["hello".to_string()],
            Some(0),
            Duration::from_millis(42),
            false,
            false,
            "success",
        );
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test-policy"));
        assert!(json.contains("echo"));
        let parsed: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "echo");
        assert_eq!(parsed.exit_code, Some(0));
    }

    #[test]
    fn test_audit_log_write() {
        let tmp = std::env::temp_dir().join("aion-sandbox-test-audit");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let log = AuditLog::new(&tmp);
        let entry = AuditLog::make_entry(
            "test",
            "ls",
            &["-la".to_string()],
            Some(0),
            Duration::from_millis(10),
            false,
            false,
            "success",
        );
        log.record(&entry).unwrap();

        let content = fs::read_to_string(tmp.join("sandbox_audit.log")).unwrap();
        assert!(content.contains("\"command\":\"ls\""));

        let _ = fs::remove_dir_all(&tmp);
    }
}
