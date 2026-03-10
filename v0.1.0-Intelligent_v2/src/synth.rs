use crate::models::{SkillMetadata, Permissions, Config};
use std::path::Path;
use std::fs;
use std::process::Command;
use anyhow::{Result, Context};

pub struct Synth;

impl Synth {
    pub fn synthesize(config: &Config, capability: &str, task: &str) -> Result<SkillMetadata> {
        let skill_name = format!("synth_{}", capability.replace("_", ""));
        println!("Synthesizing new intelligent skill: {} for task: '{}'", skill_name, task);

        let skill_dir = Path::new(&config.skills_dir).join(&skill_name);
        fs::create_dir_all(&skill_dir)?;

        // Default skeleton permissions (restrictive by default)
        let permissions = Permissions {
            network: false,
            filesystem_read: true, // Often needed for tasks
            filesystem_write: false,
            process_exec: true, // To run itself
        };

        let skill_meta = SkillMetadata {
            name: skill_name.clone(),
            version: "0.1.0".to_string(),
            capabilities: vec![capability.to_string()],
            source: Some("synth_generated".to_string()),
            permissions,
            usage: None,
            lifecycle: None,
            description: Some(format!("AI-synthesized skill for: {}", task)),
            entrypoint: Some("main.py".to_string()),
        };

        // 1. Write skill.json
        let meta_content = serde_json::to_string_pretty(&skill_meta)?;
        fs::write(skill_dir.join("skill.json"), meta_content)?;

        // 2. Generate Python logic
        let mut py_content = format!(r#"import sys
# Original Task: {}
# Auto-generated fallback logic
print("Executing capability: {}...")
sys.exit(0)
"#, task, capability);

        // Attempt to call LLM if enabled and configured
        if config.llm_enabled.unwrap_or(false) {
            if let Some(cmd_str) = &config.llm_command {
                println!("Calling LLM Generator: {}...", cmd_str);
                
                // Example call: python generator.py "the task" "the capability"
                let output = Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(format!("{} '{}' '{}'", cmd_str, task, capability))
                    .output();

                match output {
                    Ok(out) if out.status.success() => {
                        let generated_code = String::from_utf8_lossy(&out.stdout).to_string();
                        if !generated_code.trim().is_empty() {
                            py_content = generated_code;
                            println!("LLM Code Generation successful!");
                        }
                    }
                    Ok(out) => {
                        eprintln!("LLM Generator returned error: {}", String::from_utf8_lossy(&out.stderr));
                    }
                    Err(e) => {
                        eprintln!("Failed to invoke LLM Generator: {}", e);
                    }
                }
            }
        }

        // 3. Write entrypoint
        fs::write(skill_dir.join("main.py"), py_content)?;

        Ok(skill_meta)
    }
}
