use crate::models::{SkillMetadata, Permissions, Config};
use std::path::Path;
use std::fs;
use anyhow::Result;

pub struct Synth;

impl Synth {
    pub fn synthesize(config: &Config, capability: &str) -> Result<SkillMetadata> {
        let skill_name = format!("synth_{}", capability.replace("_", ""));
        println!("Synthesizing new skill: {} for capability: {}", skill_name, capability);

        let skill_dir = Path::new(&config.skills_dir).join(&skill_name);
        fs::create_dir_all(&skill_dir)?;

        let skill_meta = SkillMetadata {
            name: skill_name.clone(),
            version: "0.1.0".to_string(),
            capabilities: vec![capability.to_string()],
            source: Some("synth_generated".to_string()),
            permissions: Permissions {
                network: false, // Default rules
                filesystem_read: false,
                filesystem_write: false,
                process_exec: true,
            },
            usage: None,
            lifecycle: None,
            description: Some(format!("Auto-generated skill for {}", capability)),
            entrypoint: Some("main.py".to_string()),
        };

        // Write skill.json
        let meta_content = serde_json::to_string_pretty(&skill_meta)?;
        fs::write(skill_dir.join("skill.json"), meta_content)?;

        // Write entrypoint
        let py_content = format!(r#"import sys
print("Synthesized Skill: Executing {}...")
sys.exit(0)
"#, capability);
        fs::write(skill_dir.join("main.py"), py_content)?;

        Ok(skill_meta)
    }
}
