//! CLI 工具分析器
//!
//! 分析外部 CLI 工具的 --help 输出，提取命令结构、参数和选项。

use anyhow::Result;
use serde::{Deserialize, Serialize};

use aion_sandbox::{SandboxedCommand, SandboxedExecutor};

/// 工具分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAnalysis {
    /// 工具名称
    pub name: String,
    /// 版本号
    pub version: Option<String>,
    /// 描述
    pub description: String,
    /// 子命令列表
    pub subcommands: Vec<SubcommandInfo>,
    /// 全局选项
    pub global_options: Vec<OptionInfo>,
    /// 原始 help 文本
    pub raw_help: String,
}

/// 子命令信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcommandInfo {
    /// 命令名
    pub name: String,
    /// 描述
    pub description: String,
    /// 参数
    pub options: Vec<OptionInfo>,
}

/// 选项/参数信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionInfo {
    /// 长名称（如 --output）
    pub long: Option<String>,
    /// 短名称（如 -o）
    pub short: Option<String>,
    /// 描述
    pub description: String,
    /// 是否需要值
    pub takes_value: bool,
    /// 是否必需
    pub required: bool,
}

/// CLI 工具分析器
pub struct ToolAnalyzer;

impl ToolAnalyzer {
    /// 分析一个 CLI 工具
    pub async fn analyze(
        tool_name: &str,
        executor: &SandboxedExecutor,
    ) -> Result<ToolAnalysis> {
        // 1. 获取 --help 输出
        let help_cmd = SandboxedCommand {
            command: tool_name.to_string(),
            args: vec!["--help".to_string()],
            extra_env: Default::default(),
            work_dir: None,
        };

        let help_output = executor.execute(&help_cmd).await?;
        let help_text = if !help_output.stdout.is_empty() {
            help_output.stdout.clone()
        } else {
            help_output.stderr.clone()
        };

        // 2. 尝试获取版本
        let version_cmd = SandboxedCommand {
            command: tool_name.to_string(),
            args: vec!["--version".to_string()],
            extra_env: Default::default(),
            work_dir: None,
        };
        let version = match executor.execute(&version_cmd).await {
            Ok(v) => {
                let text = v.stdout.trim().to_string();
                if text.is_empty() { None } else { Some(text) }
            }
            Err(_) => None,
        };

        // 3. 解析 help 文本
        let (description, subcommands, global_options) = Self::parse_help_text(&help_text);

        Ok(ToolAnalysis {
            name: tool_name.to_string(),
            version,
            description,
            subcommands,
            global_options,
            raw_help: help_text,
        })
    }

    /// 解析 help 文本提取结构化信息
    fn parse_help_text(help: &str) -> (String, Vec<SubcommandInfo>, Vec<OptionInfo>) {
        let lines: Vec<&str> = help.lines().collect();
        let mut description = String::new();
        let mut subcommands = Vec::new();
        let mut options = Vec::new();

        let mut section = "header";

        for line in &lines {
            let trimmed = line.trim();

            // 检测段落
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("usage:") || lower.starts_with("用法:") {
                section = "usage";
                continue;
            }
            if lower.starts_with("commands:") || lower.starts_with("subcommands:")
                || lower.starts_with("命令:")
            {
                section = "commands";
                continue;
            }
            if lower.starts_with("options:") || lower.starts_with("flags:")
                || lower.starts_with("选项:")
            {
                section = "options";
                continue;
            }

            match section {
                "header" => {
                    if !trimmed.is_empty() && description.is_empty() {
                        description = trimmed.to_string();
                    }
                }
                "commands" => {
                    if let Some(cmd) = Self::parse_subcommand_line(trimmed) {
                        subcommands.push(cmd);
                    }
                }
                "options" => {
                    if let Some(opt) = Self::parse_option_line(trimmed) {
                        options.push(opt);
                    }
                }
                _ => {}
            }
        }

        (description, subcommands, options)
    }

    /// 解析子命令行
    fn parse_subcommand_line(line: &str) -> Option<SubcommandInfo> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // 常见格式: "  command-name    Description text"
        let parts: Vec<&str> = trimmed.splitn(2, |c: char| c.is_whitespace()).collect();
        if parts.len() < 2 {
            return Some(SubcommandInfo {
                name: parts[0].to_string(),
                description: String::new(),
                options: Vec::new(),
            });
        }

        Some(SubcommandInfo {
            name: parts[0].trim().to_string(),
            description: parts[1].trim().to_string(),
            options: Vec::new(),
        })
    }

    /// 解析选项行
    fn parse_option_line(line: &str) -> Option<OptionInfo> {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('-') {
            return None;
        }

        let mut long = None;
        let mut short = None;
        let mut takes_value = false;

        // 提取 flags 和描述
        let (flags_part, desc_part) = if let Some(idx) = trimmed.find("  ") {
            (&trimmed[..idx], trimmed[idx..].trim())
        } else {
            (trimmed, "")
        };

        for token in flags_part.split(',') {
            let token = token.trim().split_whitespace().next().unwrap_or("");
            if token.starts_with("--") {
                long = Some(token.to_string());
            } else if token.starts_with('-') {
                short = Some(token.to_string());
            }
        }

        // 如果有 <value> 或 =VALUE 说明需要值
        if flags_part.contains('<') || flags_part.contains('=') || flags_part.contains("VALUE") {
            takes_value = true;
        }

        Some(OptionInfo {
            long,
            short,
            description: desc_part.to_string(),
            takes_value,
            required: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_option_line() {
        let opt = ToolAnalyzer::parse_option_line("  -o, --output <FILE>  Output file path").unwrap();
        assert_eq!(opt.long, Some("--output".to_string()));
        assert_eq!(opt.short, Some("-o".to_string()));
        assert!(opt.takes_value);
    }

    #[test]
    fn test_parse_option_line_flag() {
        let opt = ToolAnalyzer::parse_option_line("  -v, --verbose  Enable verbose output").unwrap();
        assert_eq!(opt.long, Some("--verbose".to_string()));
        assert!(!opt.takes_value);
    }

    #[test]
    fn test_parse_subcommand_line() {
        let cmd = ToolAnalyzer::parse_subcommand_line("  build    Build the project").unwrap();
        assert_eq!(cmd.name, "build");
        assert_eq!(cmd.description, "Build the project");
    }

    #[test]
    fn test_parse_help_text() {
        let help = "My Tool v1.0\n\nUsage: mytool [OPTIONS] [COMMAND]\n\nCommands:\n  build    Build stuff\n  test     Run tests\n\nOptions:\n  -v, --verbose  Be verbose\n  -h, --help     Show help\n";
        let (desc, cmds, opts) = ToolAnalyzer::parse_help_text(help);
        assert_eq!(desc, "My Tool v1.0");
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name, "build");
        assert_eq!(opts.len(), 2);
    }
}
