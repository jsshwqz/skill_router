//! SKILL.md 清单解析
//!
//! 机器可读的技能清单结构，从 SKILL.md 文件的 YAML frontmatter 解析。
//! 灵感来源：通用的 AI Agent 技能发现模式。

use serde::{Deserialize, Serialize};

/// 命令规格定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    /// 命令名称
    pub name: String,
    /// 命令描述
    #[serde(default)]
    pub description: String,
    /// 参数列表
    #[serde(default)]
    pub args: Vec<ArgSpec>,
    /// 输出格式
    #[serde(default)]
    pub output_format: OutputFormat,
}

/// 参数规格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgSpec {
    /// 参数名称
    pub name: String,
    /// 参数类型
    #[serde(default = "default_arg_type")]
    pub arg_type: String,
    /// 参数描述
    #[serde(default)]
    pub description: String,
    /// 是否必需
    #[serde(default)]
    pub required: bool,
    /// 默认值
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
}

fn default_arg_type() -> String {
    "string".to_string()
}

/// 输入/输出参数规格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    /// 参数名称
    pub name: String,
    /// 参数类型
    #[serde(default = "default_arg_type")]
    pub param_type: String,
    /// 参数描述
    #[serde(default)]
    pub description: String,
    /// 是否必需
    #[serde(default)]
    pub required: bool,
    /// 默认值
    #[serde(default)]
    pub default_value: Option<serde_json::Value>,
}

/// 输出格式
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// JSON 结构化输出（AI Agent 首选）
    #[default]
    Json,
    /// 纯文本输出
    PlainText,
    /// 自定义结构化输出
    Structured,
}

/// 使用示例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleSpec {
    /// 示例标题
    #[serde(default)]
    pub title: String,
    /// 执行的命令/任务
    pub input: String,
    /// 预期输出
    #[serde(default)]
    pub expected_output: String,
}

/// 平台需求
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlatformRequirement {
    #[default]
    Any,
    Windows,
    Linux,
    MacOs,
    Custom(String),
}

/// 沙箱策略引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicyRef {
    /// 策略文件路径（相对于技能根目录）
    pub path: String,
    /// 策略内容哈希（用于完整性验证）
    #[serde(default)]
    pub hash: Option<String>,
}

/// SKILL.md 清单——AI Agent 技能发现的核心数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// 技能名称
    pub name: String,
    /// 版本号
    #[serde(default = "default_version")]
    pub version: String,
    /// 技能描述
    #[serde(default)]
    pub description: String,
    /// 提供的能力列表
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// 命令定义
    #[serde(default)]
    pub commands: Vec<CommandSpec>,
    /// 输入参数
    #[serde(default)]
    pub inputs: Vec<ParamSpec>,
    /// 输出参数
    #[serde(default)]
    pub outputs: Vec<ParamSpec>,
    /// 使用示例
    #[serde(default)]
    pub examples: Vec<ExampleSpec>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 平台需求
    #[serde(default)]
    pub platform: PlatformRequirement,
    /// 外部依赖
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// 沙箱策略引用
    #[serde(default)]
    pub sandbox_policy: Option<SandboxPolicyRef>,
    /// 入口点（可执行文件名或 builtin: 前缀）
    #[serde(default)]
    pub entrypoint: Option<String>,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

impl SkillManifest {
    /// 从 SKILL.md 文件的 YAML frontmatter 解析清单
    ///
    /// SKILL.md 格式：
    /// ```text
    /// ---
    /// name: my-skill
    /// version: 1.0.0
    /// capabilities: [text_translate, text_summarize]
    /// ...
    /// ---
    ///
    /// # My Skill
    /// Human-readable documentation here.
    /// ```
    pub fn parse_from_skill_md(content: &str) -> anyhow::Result<Self> {
        let content = content.trim();
        if !content.starts_with("---") {
            return Err(anyhow::anyhow!("SKILL.md must start with YAML frontmatter delimiter '---'"));
        }

        // 找到第二个 ---
        let rest = &content[3..];
        let end = rest.find("---").ok_or_else(|| {
            anyhow::anyhow!("SKILL.md missing closing YAML frontmatter delimiter '---'")
        })?;

        let yaml_str = rest[..end].trim();

        // 手动解析简单 YAML（不引入 yaml crate，用 serde_json 转换）
        let json_value = simple_yaml_to_json(yaml_str)?;
        let manifest: SkillManifest = serde_json::from_value(json_value)?;
        Ok(manifest)
    }
}

/// 简易 YAML → JSON 转换（支持基本的 key: value 和数组格式）
fn simple_yaml_to_json(yaml: &str) -> anyhow::Result<serde_json::Value> {
    let mut map = serde_json::Map::new();

    for line in yaml.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim();

            if value.is_empty() {
                // 可能是多行值，暂时设为 null
                map.insert(key, serde_json::Value::Null);
            } else if value.starts_with('[') && value.ends_with(']') {
                // 内联数组 [a, b, c]
                let inner = &value[1..value.len() - 1];
                let items: Vec<serde_json::Value> = inner
                    .split(',')
                    .map(|s| serde_json::Value::String(s.trim().to_string()))
                    .collect();
                map.insert(key, serde_json::Value::Array(items));
            } else if value == "true" {
                map.insert(key, serde_json::Value::Bool(true));
            } else if value == "false" {
                map.insert(key, serde_json::Value::Bool(false));
            } else if let Ok(n) = value.parse::<i64>() {
                map.insert(key, serde_json::json!(n));
            } else if let Ok(f) = value.parse::<f64>() {
                map.insert(key, serde_json::json!(f));
            } else {
                map.insert(key, serde_json::Value::String(value.to_string()));
            }
        }
    }

    Ok(serde_json::Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_md() {
        let content = r#"---
name: curl-wrapper
version: 1.0.0
description: HTTP request tool
capabilities: [http_fetch, web_search]
platform: any
---

# curl-wrapper

A sandboxed curl wrapper for AI agents.
"#;
        let manifest = SkillManifest::parse_from_skill_md(content).unwrap();
        assert_eq!(manifest.name, "curl-wrapper");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.capabilities.len(), 2);
        assert_eq!(manifest.capabilities[0], "http_fetch");
    }

    #[test]
    fn test_parse_skill_md_missing_frontmatter() {
        let content = "# No frontmatter";
        assert!(SkillManifest::parse_from_skill_md(content).is_err());
    }

    #[test]
    fn test_simple_yaml_to_json() {
        let yaml = "name: test\nversion: 1.0\nenabled: true\ncount: 42";
        let json = simple_yaml_to_json(yaml).unwrap();
        assert_eq!(json["name"], "test");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["count"], 42);
    }

    #[test]
    fn test_skill_manifest_serde() {
        let manifest = SkillManifest {
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            description: "A test skill".to_string(),
            capabilities: vec!["echo".to_string()],
            commands: vec![CommandSpec {
                name: "echo".to_string(),
                description: "Echo text".to_string(),
                args: vec![ArgSpec {
                    name: "text".to_string(),
                    arg_type: "string".to_string(),
                    description: "Text to echo".to_string(),
                    required: true,
                    default_value: None,
                }],
                output_format: OutputFormat::Json,
            }],
            inputs: vec![],
            outputs: vec![],
            examples: vec![ExampleSpec {
                title: "Basic echo".to_string(),
                input: "echo hello".to_string(),
                expected_output: "hello".to_string(),
            }],
            tags: vec!["test".to_string()],
            platform: PlatformRequirement::Any,
            dependencies: vec![],
            sandbox_policy: None,
            entrypoint: Some("echo".to_string()),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: SkillManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test-skill");
        assert_eq!(parsed.commands.len(), 1);
    }
}
