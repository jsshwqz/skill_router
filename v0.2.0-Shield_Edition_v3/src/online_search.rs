use crate::models::{SkillMetadata, Permissions, Config};
use std::process::Command;
use serde_json::Value;

pub struct OnlineSearch;

impl OnlineSearch {
    pub fn search(config: &Config, capability: &str, task: &str) -> Option<SkillMetadata> {
        println!("[ONLINE SEARCH] Looking for capability '{}' on GitHub/Web...", capability);
        
        // 1. Call Search Agent to find real skill repositories or technical docs
        let output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!("python search_agent.py '{}' '{}'", task, capability))
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                if let Ok(json_res) = serde_json::from_str::<Value>(&stdout) {
                    
                    // Case A: Found a real skill repository on GitHub
                    if json_res["source"] == "github" {
                        let skill_name = json_res["name"].as_str().unwrap_or("github_skill");
                        let clone_url = json_res["url"].as_str().unwrap_or("");
                        println!("Found match on GitHub: {}. Preparing installation...", clone_url);
                        
                        // Installation flow: git clone [clone_url] skills/[skill_name]
                        if !clone_url.is_empty() {
                            let install_res = Command::new("git")
                                .arg("clone")
                                .arg(clone_url)
                                .arg(format!("{}/{}", config.skills_dir, skill_name))
                                .output();
                                
                            if install_res.is_ok() && install_res.unwrap().status.success() {
                                println!("Skill successfully cloned. Starting Rust-native Safety Analysis...");
                                
                                // Call Rust-native Security Auditor
                                let skill_dir = std::path::Path::new(&config.skills_dir).join(skill_name);
                                
                                match crate::security_analyzer::SecurityAnalyzer::audit_skill_dir(&skill_dir) {
                                    Ok(_) => {
                                        println!("Safety Analysis Passed. Registering skill.");
                                        if let Ok(content) = std::fs::read_to_string(skill_dir.join("skill.json")) {
                                            if let Ok(meta) = serde_json::from_str::<SkillMetadata>(&content) {
                                                return Some(meta);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("SECURITY ALERT: Skill '{}' failed safety analysis: {}. Deleting for safety.", skill_name, e);
                                        let _ = std::fs::remove_dir_all(skill_dir);
                                    }
                                }
                            }
                        }
                    } 
                    
                    // Case B: No skill repo, but got technical documentation for Synthesis
                    else if json_res["source"] == "web_docs" {
                        println!("No ready-to-use skill repo. Technical docs found. Triggering Synthesis...");
                        // We return None here so the main loop will proceed to Synth module
                        // The Synth module will then call the LLM with the search context
                        return None;
                    }
                }
            }
            _ => eprintln!("Search Agent failed or returned invalid response."),
        }
        
        None
    }
}
