use crate::models::{SkillMetadata, Permissions, Config};
use std::path::Path;
use std::fs;
use std::process::Command;
use anyhow::Result;
use serde_json::Value;

pub struct Synth;

impl Synth {
    pub fn synthesize(config: &Config, capability: &str, task: &str) -> Result<SkillMetadata> {
        let skill_name = format!("synth_{}", capability.replace("_", ""));
        let skill_dir = Path::new(&config.skills_dir).join(&skill_name);
        fs::create_dir_all(&skill_dir)?;

        let mut entrypoint = "main.py".to_string();
        let mut py_content = format!("print('Fallback for {}')", task);

        // Call AI Generator (Preferring Rust)
        if config.llm_enabled.unwrap_or(false) {
            if let Some(cmd_str) = &config.llm_command {
                let output = Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(format!("{} '{}' '{}'", cmd_str, task, capability))
                    .output();

                if let Ok(out) = output {
                    if out.status.success() {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        if let Ok(json_res) = serde_json::from_str::<Value>(&stdout) {
                            if json_res["language"] == "rust" {
                                println!("[SYNTH] Generating RUST skill...");
                                // Write Cargo.toml
                                fs::write(skill_dir.join("Cargo.toml"), json_res["cargo_toml"].as_str().unwrap())?;
                                // Write src/main.rs
                                let src_dir = skill_dir.join("src");
                                fs::create_dir_all(&src_dir)?;
                                fs::write(src_dir.join("main.rs"), json_res["main_rs"].as_str().unwrap())?;
                                
                                // Attempt compilation
                                println!("[BUILD] Compiling Rust skill with cargo...");
                                let build_res = Command::new("cargo")
                                    .arg("build")
                                    .arg("--release")
                                    .current_dir(&skill_dir)
                                    .output();
                                    
                                if build_res.is_ok() && build_res.unwrap().status.success() {
                                    entrypoint = format!("target/release/synth_{}.exe", capability.replace("_", ""));
                                    println!("[BUILD] Rust compilation successful!");
                                } else {
                                    println!("[BUILD] Rust compilation failed. Falling back to python skeleton.");
                                }
                            } else {
                                py_content = json_res["code"].as_str().unwrap_or("").to_string();
                            }
                        }
                    }
                }
            }
        }

        // Write skill.json
        let skill_meta = SkillMetadata {
            name: skill_name.clone(),
            version: "0.0.1".to_string(),
            capabilities: vec![capability.to_string()],
            source: Some("synth_generated".to_string()),
            permissions: Permissions {
                network: false,
                filesystem_read: true,
                filesystem_write: true,
                process_exec: true,
            },
            usage: None,
            lifecycle: None,
            description: Some(format!("Rust-preferred AI skill for: {}", task)),
            entrypoint: Some(entrypoint.clone()),
        };
        fs::write(skill_dir.join("skill.json"), serde_json::to_string_pretty(&skill_meta)?)?;

        if entrypoint.ends_with(".py") {
             fs::write(skill_dir.join("main.py"), py_content)?;
        }

        Ok(skill_meta)
    }
}
