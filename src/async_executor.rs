use crate::models::{Config, SkillMetadata};
use crate::security::Security;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::Path;
use tokio::process::Command as TokioCommand;

pub struct AsyncExecutor;

impl AsyncExecutor {
    /// 异步执行技能，返回执行句柄
    pub async fn execute_async(config: &Config, skill: &SkillMetadata) -> Result<tokio::task::JoinHandle<Result<()>>> {
        // Security check for execution
        Security::validate_permissions(&skill.name, "process_exec", &skill.permissions)?;

        // 支持 path 字段，允许技能在其他位置
        let skill_dir = if let Some(ref skill_path) = skill.path {
            Path::new(skill_path)
                .canonicalize()
                .with_context(|| format!("Failed to resolve skill path: {}", skill_path))?
        } else {
            Path::new(&config.skills_dir)
                .canonicalize()
                .with_context(|| "Failed to resolve skills directory path")?
                .join(&skill.name)
        };

        let raw_entrypoint = skill.entrypoint.as_deref().unwrap_or("main.rs");
        
        // 处理 Windows 路径分隔符
        let raw_entrypoint_normalized = raw_entrypoint.replace("\\", "/");
        
        // 如果 entrypoint 是相对路径，基于 skill_dir 解析
        let entry_path = if Path::new(&raw_entrypoint_normalized).is_relative() {
            skill_dir.join(&raw_entrypoint_normalized)
        } else {
            Path::new(&raw_entrypoint_normalized).to_path_buf()
        };

        // Safety check: Prevent directory traversal (e.g. entrypoint: "../../etc/passwd")
        if skill.path.is_none() {
            // 只有在使用 skills_dir 时才检查路径遍历
            if !entry_path.starts_with(&skill_dir) {
                bail!(
                    "Security Violation: Skill '{}' entrypoint lies outside its directory: {:?}",
                    skill.name,
                    entry_path
                );
            }
        }

        if !entry_path.exists() {
            bail!("Skill entrypoint not found: {:?}", entry_path);
        }

        // 克隆必要的数据用于异步任务
        let skill_name = skill.name.clone();
        let config_clone = config.clone();
        let skill_dir_clone = skill_dir.clone();
        let entry_path_clone = entry_path.clone();
        let raw_entrypoint_clone = raw_entrypoint.to_string();

        // 创建异步任务
        let handle = tokio::spawn(async move {
            Self::execute_skill(
                &config_clone,
                &skill_name,
                &skill_dir_clone,
                &entry_path_clone,
                &raw_entrypoint_clone,
            ).await
        });

        Ok(handle)
    }

    async fn execute_skill(
        config: &Config,
        skill_name: &str,
        skill_dir: &Path,
        entry_path: &Path,
        raw_entrypoint: &str,
    ) -> Result<()> {
        let mut command = if raw_entrypoint.ends_with(".py") {
            let mut cmd = TokioCommand::new("python");
            cmd.arg(raw_entrypoint); // Entrypoint as relative path from CWD
            cmd
        } else if raw_entrypoint.ends_with(".rs") {
            // For Rust skills, use cargo run
            let mut cmd = TokioCommand::new("cargo");
            cmd.arg("run");
            cmd.arg("--release");
            cmd.current_dir(skill_dir); // Execute inside skill directory context
            cmd
        } else {
            // 对于 .exe 或其他可执行文件，使用完整路径
            TokioCommand::new(entry_path)
        };

        let output = if raw_entrypoint.ends_with(".rs") {
            // cargo run handles its own directory context
            command
                .output()
                .await
                .with_context(|| format!("Failed to execute skill '{}'", skill_name))?
        } else {
            command
                .current_dir(skill_dir) // Execute inside skill directory context
                .output()
                .await
                .with_context(|| format!("Failed to execute skill '{}'", skill_name))?
        };

        // Task 8: Execution logging
        let log_dir = Path::new(&config.logs_dir);
        if !log_dir.exists() {
            fs::create_dir_all(log_dir)?;
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let log_file = log_dir.join(format!("{}_{}.log", skill_name, timestamp));

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let log_content = format!(
            "Timestamp: {}\nSkill: {}\nStatus: {}\nStdout:\n{}\nStderr:\n{}\n",
            Utc::now().to_rfc3339(),
            skill_name,
            output.status,
            stdout,
            stderr
        );
        fs::write(&log_file, log_content)?;

        if output.status.success() {
            Ok(())
        } else {
            bail!(
                "Skill '{}' failed with status {}. Stderr: {}",
                skill_name,
                output.status,
                stderr
            );
        }
    }

    /// 批量异步执行多个技能（顺序执行，但每个技能异步）
    pub async fn execute_batch_async(
        config: &Config,
        skills: Vec<&SkillMetadata>,
    ) -> Result<Vec<tokio::task::JoinHandle<Result<()>>>> {
        let mut handles = Vec::new();
        
        for skill in skills {
            let handle = Self::execute_async(config, skill).await?;
            handles.push(handle);
        }

        Ok(handles)
    }

    /// 带超时的异步执行
    pub async fn execute_with_timeout(
        config: &Config,
        skill: &SkillMetadata,
        timeout_secs: u64,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let handle = Self::execute_async(config, skill).await?;
        
        // 返回原始执行句柄，超时逻辑由调用者处理
        Ok(handle)
    }
}