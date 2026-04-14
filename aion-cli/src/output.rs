//! 输出格式抽象
//!
//! 根据 `--json`/`--quiet` 标志选择输出模式：
//! - Pretty：带 spinner + 彩色 stderr 提示
//! - Json：纯 JSON 输出到 stdout，stderr 静默
//! - Quiet：仅 stdout 结果，无额外输出

use indicatif::{ProgressBar, ProgressStyle};

/// 输出模式
#[derive(Clone, Copy, Debug)]
pub enum OutputMode {
    /// 默认模式：带 spinner 和彩色提示
    Pretty,
    /// 机器可读模式：纯 JSON 输出
    Json,
    /// 安静模式：仅结果
    Quiet,
}

/// 进度报告器
pub struct ProgressReporter {
    mode: OutputMode,
}

impl ProgressReporter {
    pub fn new(mode: OutputMode) -> Self {
        Self { mode }
    }

    /// 显示信息消息（仅 Pretty 模式）
    pub fn info(&self, msg: &str) {
        if matches!(self.mode, OutputMode::Pretty) {
            eprintln!("{}", msg);
        }
    }

    /// 创建路由 spinner（仅 Pretty 模式返回 Some）
    pub fn routing_spinner(&self) -> Option<ProgressBar> {
        match self.mode {
            OutputMode::Pretty => {
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.cyan} {msg}")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.set_message("Routing task...");
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(pb)
            }
            _ => None,
        }
    }

    /// 创建多步骤进度条（仅 Pretty 模式）
    pub fn pipeline_progress(&self, steps: u64) -> Option<ProgressBar> {
        match self.mode {
            OutputMode::Pretty => {
                let pb = ProgressBar::new(steps);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{bar:30.cyan/dim}] {pos}/{len} {msg}")
                        .unwrap_or_else(|_| ProgressStyle::default_bar())
                        .progress_chars("=> "),
                );
                Some(pb)
            }
            _ => None,
        }
    }

    /// 报告成功结果
    pub fn finish_success(
        &self,
        spinner: Option<ProgressBar>,
        skill_name: &str,
        status: &str,
        lifecycle: &str,
        result: &serde_json::Value,
    ) {
        if let Some(pb) = spinner {
            pb.finish_and_clear();
        }

        match self.mode {
            OutputMode::Pretty => {
                eprintln!("Skill: {} | Status: {}", skill_name, status);
                eprintln!("Lifecycle: {}", lifecycle);
                if !result.is_null() {
                    println!("{}", serde_json::to_string_pretty(result).unwrap_or_default());
                }
            }
            OutputMode::Json => {
                let output = serde_json::json!({
                    "status": status,
                    "skill": skill_name,
                    "lifecycle": lifecycle,
                    "result": result,
                });
                println!("{}", serde_json::to_string(&output).unwrap_or_default());
            }
            OutputMode::Quiet => {
                if !result.is_null() {
                    println!("{}", serde_json::to_string_pretty(result).unwrap_or_default());
                }
            }
        }
    }

    /// 报告失败
    pub fn finish_error(&self, spinner: Option<ProgressBar>, error: &str) {
        if let Some(pb) = spinner {
            pb.finish_and_clear();
        }

        match self.mode {
            OutputMode::Pretty => {
                eprintln!("Error: {}", error);
            }
            OutputMode::Json => {
                let output = serde_json::json!({
                    "status": "error",
                    "error": error,
                });
                println!("{}", serde_json::to_string(&output).unwrap_or_default());
            }
            OutputMode::Quiet => {
                eprintln!("{}", error);
            }
        }
    }
}
