use crate::models::{Config, SkillMetadata};
use crate::security::Security;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct Executor;

impl Executor {
    pub fn execute(config: &Config, skill: &SkillMetadata, silent: bool) -> Result<()> {
        if !silent {
            println!("Preparing execution for skill: {}", skill.name);
        }

        // Security check for execution
        Security::validate_permissions(&skill.name, "process_exec", &skill.permissions)?;

        let skill_dir = Path::new(&config.skills_dir)
            .canonicalize()
            .with_context(|| "Failed to resolve skills directory path")?
            .join(&skill.name);

        let raw_entrypoint = skill.entrypoint.as_deref().unwrap_or("main.rs");

        // Safety check: Prevent directory traversal (e.g. entrypoint: "../../etc/passwd")
        let entry_path = skill_dir.join(raw_entrypoint);
        if !entry_path.starts_with(&skill_dir) {
            bail!(
                "Security Violation: Skill '{}' entrypoint lies outside its directory: {:?}",
                skill.name,
                entry_path
            );
        }

        if !entry_path.exists() {
            bail!("Skill entrypoint not found: {:?}", entry_path);
        }

        if !silent {
            println!("Spawning process: {:?}", entry_path);
        }

        // Secure command execution
        let mut command = if raw_entrypoint.ends_with(".py") {
            let mut cmd = Command::new("python");
            cmd.arg(raw_entrypoint); // Entrypoint as relative path from CWD
            cmd
        } else if raw_entrypoint.ends_with(".rs") {
            // For Rust skills, use cargo run
            let mut cmd = Command::new("cargo");
            cmd.arg("run");
            cmd.arg("--release");
            cmd.current_dir(&skill_dir); // Execute inside skill directory context
            cmd
        } else {
            Command::new(raw_entrypoint)
        };

        let output = if raw_entrypoint.ends_with(".rs") {
            // cargo run handles its own directory context
            command
                .output()
                .with_context(|| format!("Failed to execute skill '{}'", skill.name))?
        } else {
            command
                .current_dir(&skill_dir) // Execute inside skill directory context
                .output()
                .with_context(|| format!("Failed to execute skill '{}'", skill.name))?
        };

        // Task 8: Execution logging
        let log_dir = Path::new(&config.logs_dir);
        if !log_dir.exists() {
            fs::create_dir_all(log_dir)?;
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let log_file = log_dir.join(format!("{}_{}.log", skill.name, timestamp));

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let log_content = format!(
            "Timestamp: {}\nSkill: {}\nStatus: {}\nStdout:\n{}\nStderr:\n{}\n",
            Utc::now().to_rfc3339(),
            skill.name,
            output.status,
            stdout,
            stderr
        );
        fs::write(&log_file, log_content)?;

        if output.status.success() {
            if !silent {
                println!("Skill '{}' executed successfully.", skill.name);
                println!("Log written to: {:?}", log_file);
            }
            Ok(())
        } else {
            bail!(
                "Skill '{}' failed with status {}. Stderr: {}",
                skill.name,
                output.status,
                stderr
            );
        }
    }
}
