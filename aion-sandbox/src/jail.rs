//! 资源限制（Jail）
//!
//! 对沙箱化执行施加资源约束：超时、输出上限、环境变量过滤。

use std::collections::HashMap;
use std::time::Duration;

/// 执行资源限制配置
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// 最大执行时间
    pub timeout: Duration,
    /// stdout + stderr 最大收集字节数
    pub max_output_bytes: usize,
    /// 允许透传的环境变量
    pub allowed_env_vars: Vec<String>,
}

impl ResourceLimits {
    /// 从当前进程环境中过滤出允许的变量
    pub fn filtered_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        for key in &self.allowed_env_vars {
            if let Ok(val) = std::env::var(key) {
                env.insert(key.clone(), val);
            }
        }
        env
    }
}

/// 截断缓冲区到指定字节数，返回 (截断后内容, 是否被截断)
pub fn truncate_output(data: &[u8], max_bytes: usize) -> (Vec<u8>, bool) {
    if data.len() <= max_bytes {
        (data.to_vec(), false)
    } else {
        let mut truncated = data[..max_bytes].to_vec();
        truncated.extend_from_slice(b"\n... [output truncated]");
        (truncated, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_within_limit() {
        let data = b"hello world";
        let (result, truncated) = truncate_output(data, 100);
        assert_eq!(result, data.to_vec());
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_exceeds_limit() {
        let data = b"hello world, this is a long output";
        let (result, truncated) = truncate_output(data, 5);
        assert!(truncated);
        assert!(result.starts_with(b"hello"));
        assert!(String::from_utf8_lossy(&result).contains("truncated"));
    }

    #[test]
    fn test_filtered_env() {
        std::env::set_var("SANDBOX_TEST_VAR", "test_value");
        let limits = ResourceLimits {
            timeout: Duration::from_secs(10),
            max_output_bytes: 1024,
            allowed_env_vars: vec![
                "SANDBOX_TEST_VAR".to_string(),
                "NONEXISTENT_VAR".to_string(),
            ],
        };
        let env = limits.filtered_env();
        assert_eq!(env.get("SANDBOX_TEST_VAR"), Some(&"test_value".to_string()));
        assert!(!env.contains_key("NONEXISTENT_VAR"));
        std::env::remove_var("SANDBOX_TEST_VAR");
    }
}
