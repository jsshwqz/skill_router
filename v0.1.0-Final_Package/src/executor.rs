use std::path::Path;
use std::process::Command;
use std::fs;
use anyhow::{Result, Context, bail};
use crate::models::{SkillMetadata, Config};
use crate::security::Security;
use chrono::Utc;

pub struct Executor;

impl Executor {
    pub fn execute(config: &Config, skill: &SkillMetadata, silent: bool) -> Result<()> {
        if !silent { println!("Preparing execution for skill: {}", skill.name); }

        // Security check for execution
        Security::validate_permissions(&skill.name, "process_exec", &skill.permissions)?;

        let skill_dir = Path::new(&config.skills_dir).join(&skill.name);
        let entrypoint = skill.entrypoint.as_deref().unwrap_or("main.py");
        let entry_path = skill_dir.join(entrypoint);

        if !entry_path.exists() {
            bail!("Skill entrypoint not found: {:?}", entry_path);
        }

        if !silent { println!("Spawning process: {:?}", entry_path); }

        // Basic execution logic: assume python for .py, or execute directly if it's binary/exe.
        let mut command = if entrypoint.ends_with(".py") {
            let mut cmd = Command::new("python");
            cmd.arg(entrypoint);
            cmd
        } else {
            Command::new(entrypoint)
        };

        let output = command
            .current_dir(skill_dir)
            .output()
            .with_context(|| format!("Failed to execute skill '{}'", skill.name))?;

        // Task 8: Add execution logging
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
            bail!("Skill '{}' failed with status {}. Stderr: {}", skill.name, output.status, stderr);
        }
    }
}
