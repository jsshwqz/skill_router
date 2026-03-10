use std::path::{Path, PathBuf};
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

        let skill_dir = Path::new(&config.skills_dir).canonicalize().with_context(|| "Failed to resolve skills directory path")?.join(&skill.name);
        
        let raw_entrypoint = skill.entrypoint.as_deref().unwrap_or("main.py");
        
        // Safety check: Prevent directory traversal (e.g. entrypoint: "../../etc/passwd")
        let entry_path = skill_dir.join(raw_entrypoint);
        if !entry_path.starts_with(&skill_dir) {
             bail!("Security Violation: Skill '{}' entrypoint lies outside its directory: {:?}", skill.name, entry_path);
        }

        if !entry_path.exists() {
            bail!("Skill entrypoint not found: {:?}", entry_path);
        }

        if !silent { println!("Spawning process: {:?}", entry_path); }

        // Secure command execution
        let mut command = if raw_entrypoint.ends_with(".py") {
            let mut cmd = Command::new("python");
            cmd.arg(raw_entrypoint); // Entrypoint as relative path from CWD
            cmd
        } else {
            Command::new(raw_entrypoint)
        };

        let output = command
            .current_dir(&skill_dir) // Execute inside skill directory context
            .output()
            .with_context(|| format!("Failed to execute skill '{}'", skill.name))?;

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
            bail!("Skill '{}' failed with status {}. Stderr: {}", skill.name, output.status, stderr);
        }
    }
}
