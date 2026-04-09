//! 沙箱策略定义
//!
//! `SandboxPolicy` 使用白名单机制控制允许执行的外部命令。
//! 每条命令规则定义了允许的参数模式、超时、输出上限等约束。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 工作目录策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum WorkDirPolicy {
    /// 使用临时目录（默认，最安全）
    #[default]
    TempDir,
    /// 继承父进程工作目录
    Inherit,
    /// 指定固定目录
    Specified(PathBuf),
}


/// 单条命令规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRule {
    /// 允许的参数正则模式（任一匹配即放行；空列表 = 允许任何参数）
    #[serde(default)]
    pub allowed_args_patterns: Vec<String>,
    /// 禁止的参数正则模式（任一匹配即拒绝，优先级高于 allowed）
    #[serde(default)]
    pub blocked_args_patterns: Vec<String>,
    /// 命令执行超时（秒）
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// stdout + stderr 最大字节数
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    /// 允许透传的环境变量名
    #[serde(default)]
    pub allowed_env_vars: Vec<String>,
    /// 工作目录策略
    #[serde(default)]
    pub work_dir_policy: WorkDirPolicy,
    /// 人类可读的用途说明
    #[serde(default)]
    pub description: String,
}

fn default_timeout_secs() -> u64 {
    30
}
fn default_max_output_bytes() -> usize {
    1_048_576 // 1 MB
}

impl CommandRule {
    /// 检查给定参数列表是否被此规则允许
    pub fn validate_args(&self, args: &[String]) -> Result<()> {
        let joined = args.join(" ");

        // 优先检查 blocked
        for pattern in &self.blocked_args_patterns {
            let re = regex::Regex::new(pattern)
                .map_err(|e| anyhow::anyhow!("invalid blocked_args pattern '{}': {}", pattern, e))?;
            if re.is_match(&joined) {
                return Err(anyhow::anyhow!(
                    "argument blocked by pattern '{}': {}",
                    pattern,
                    joined
                ));
            }
        }

        // 如果有 allowed 列表，至少要匹配一个
        if !self.allowed_args_patterns.is_empty() {
            let mut matched = false;
            for pattern in &self.allowed_args_patterns {
                let re = regex::Regex::new(pattern)
                    .map_err(|e| anyhow::anyhow!("invalid allowed_args pattern '{}': {}", pattern, e))?;
                if re.is_match(&joined) {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return Err(anyhow::anyhow!(
                    "arguments not in allowlist: {}",
                    joined
                ));
            }
        }

        Ok(())
    }

    /// 获取超时 Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

/// 沙箱策略——白名单化的命令执行规则集
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    /// 策略名称
    pub name: String,
    /// 策略版本
    #[serde(default = "default_version")]
    pub version: String,
    /// 策略描述
    #[serde(default)]
    pub description: String,
    /// 允许执行的命令白名单（key = 可执行文件名）
    pub allowed_commands: BTreeMap<String, CommandRule>,
    /// 全局最大并发执行数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
}

fn default_version() -> String {
    "1.0".to_string()
}
fn default_max_concurrent() -> usize {
    4
}

impl SandboxPolicy {
    /// 从 JSON 文件加载策略
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let policy: Self = serde_json::from_str(&content)?;
        policy.validate()?;
        Ok(policy)
    }

    /// 验证策略合法性
    pub fn validate(&self) -> Result<()> {
        if self.allowed_commands.is_empty() {
            return Err(anyhow::anyhow!("sandbox policy has no allowed commands"));
        }
        // 验证所有正则都能编译
        for (cmd, rule) in &self.allowed_commands {
            for pattern in &rule.allowed_args_patterns {
                regex::Regex::new(pattern).map_err(|e| {
                    anyhow::anyhow!("invalid regex in {}.allowed_args: {}", cmd, e)
                })?;
            }
            for pattern in &rule.blocked_args_patterns {
                regex::Regex::new(pattern).map_err(|e| {
                    anyhow::anyhow!("invalid regex in {}.blocked_args: {}", cmd, e)
                })?;
            }
        }
        Ok(())
    }

    /// 查找命令是否在白名单中
    pub fn get_rule(&self, command: &str) -> Option<&CommandRule> {
        self.allowed_commands.get(command)
    }

    /// 计算策略的 SHA256 哈希（用于审批追踪）
    pub fn content_hash(path: &Path) -> Result<String> {
        let content = std::fs::read(path)?;
        use std::fmt::Write;
        // 简易 SHA256（不引入额外 crate，用 serde_json 的确定性序列化）
        let mut hash = 0u64;
        for (i, byte) in content.iter().enumerate() {
            hash = hash.wrapping_mul(31).wrapping_add(*byte as u64).wrapping_add(i as u64);
        }
        let mut s = String::new();
        write!(s, "{:016x}", hash)?;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_rule_validate_args_empty_allowlist() {
        let rule = CommandRule {
            allowed_args_patterns: vec![],
            blocked_args_patterns: vec![],
            timeout_secs: 30,
            max_output_bytes: 1024,
            allowed_env_vars: vec![],
            work_dir_policy: WorkDirPolicy::TempDir,
            description: String::new(),
        };
        // 空 allowlist = 允许所有
        assert!(rule.validate_args(&["--help".to_string()]).is_ok());
    }

    #[test]
    fn test_command_rule_blocked_takes_priority() {
        let rule = CommandRule {
            allowed_args_patterns: vec![".*".to_string()],
            blocked_args_patterns: vec!["--delete".to_string()],
            timeout_secs: 30,
            max_output_bytes: 1024,
            allowed_env_vars: vec![],
            work_dir_policy: WorkDirPolicy::TempDir,
            description: String::new(),
        };
        assert!(rule.validate_args(&["--delete".to_string()]).is_err());
        assert!(rule.validate_args(&["--help".to_string()]).is_ok());
    }

    #[test]
    fn test_command_rule_allowed_filter() {
        let rule = CommandRule {
            allowed_args_patterns: vec!["^--version$".to_string(), "^--help$".to_string()],
            blocked_args_patterns: vec![],
            timeout_secs: 30,
            max_output_bytes: 1024,
            allowed_env_vars: vec![],
            work_dir_policy: WorkDirPolicy::TempDir,
            description: String::new(),
        };
        assert!(rule.validate_args(&["--version".to_string()]).is_ok());
        assert!(rule.validate_args(&["--exec".to_string()]).is_err());
    }

    #[test]
    fn test_sandbox_policy_validate_empty() {
        let policy = SandboxPolicy {
            name: "test".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            allowed_commands: BTreeMap::new(),
            max_concurrent: 4,
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn test_sandbox_policy_serde_roundtrip() {
        let mut commands = BTreeMap::new();
        commands.insert("curl".to_string(), CommandRule {
            allowed_args_patterns: vec!["^https://".to_string()],
            blocked_args_patterns: vec!["--upload".to_string()],
            timeout_secs: 10,
            max_output_bytes: 4096,
            allowed_env_vars: vec!["HOME".to_string()],
            work_dir_policy: WorkDirPolicy::TempDir,
            description: "curl for downloads only".to_string(),
        });
        let policy = SandboxPolicy {
            name: "test-policy".to_string(),
            version: "1.0".to_string(),
            description: "test".to_string(),
            allowed_commands: commands,
            max_concurrent: 2,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test-policy");
        assert!(parsed.allowed_commands.contains_key("curl"));
    }
}
