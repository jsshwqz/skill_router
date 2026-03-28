use std::fs;

use anyhow::Result;
use serde_json::json;

use aion_types::types::{PermissionSet, RouterPaths, SkillDefinition, SkillMetadata, SkillSource};

pub struct Synthesizer;

impl Synthesizer {
    pub fn placeholder_definition(
        paths: &RouterPaths,
        capability: &str,
        _task: &str,
    ) -> Result<SkillDefinition> {
        let root_dir = paths
            .generated_skills_dir
            .join(format!("{capability}_placeholder"));
        Ok(SkillDefinition {
            metadata: SkillMetadata {
                name: format!("{capability}_placeholder"),
                version: "0.1.0".to_string(),
                capabilities: vec![capability.to_string()],
                entrypoint: "builtin:placeholder".to_string(),
                permissions: PermissionSet::default_deny(),
            },
            root_dir,
            source: SkillSource::Generated,
        })
    }

    pub fn create_placeholder(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
    ) -> Result<SkillDefinition> {
        Self::create_placeholder_with_context(paths, capability, task, None)
    }

    pub fn create_placeholder_with_context(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
        discovery_context: Option<serde_json::Value>,
    ) -> Result<SkillDefinition> {
        let definition = Self::placeholder_definition(paths, capability, task)?;
        Self::persist_definition(&definition)?;
        
        let mut readme_content = format!(
            "# {}\n\nGenerated locally for capability `{}` from task `{}`.\n",
            definition.metadata.name, capability, task
        );

        if let Some(ctx) = discovery_context {
            readme_content.push_str("\n## Discovery Intelligence\n");
            readme_content.push_str("Found related knowledge during evolution phase:\n\n");
            if let Some(hits) = ctx["hits"].as_array() {
                for hit in hits.iter().take(3) {
                    readme_content.push_str(&format!(
                        "- **{}**: {} (Source: {:?})\n", 
                        hit["title"].as_str().unwrap_or("Untitled"),
                        hit["snippet"].as_str().unwrap_or("..."),
                        hit["source"]
                    ));
                }
            }
        }

        fs::write(definition.root_dir.join("README.md"), readme_content)?;
        Ok(definition)
    }

    pub fn evolve(
        paths: &RouterPaths,
        capability: &str,
        task: &str,
        requirement: &str,
    ) -> Result<SkillDefinition> {
        // 1. Generate definition
        let name = format!("{}_evolved", capability);
        let root_dir = paths.generated_skills_dir.join(&name);
        let definition = SkillDefinition {
            metadata: SkillMetadata {
                name,
                version: "0.1.0".to_string(),
                capabilities: vec![capability.to_string()],
                entrypoint: "main.rs".to_string(),
                permissions: PermissionSet::default_deny(),
            },
            root_dir,
            source: SkillSource::Generated,
        };

        // 2. Persist metadata
        Self::persist_definition(&definition)?;

        // 3. Generate Logic (In a real system, this would call LLM)
        let code = format!(
            "// Automatically evolved skill for {}\n\
            fn main() {{\n\
                println!(\"Executing evolved logic for task: {}\");\n\
                // Requirement: {}\n\
            }}", 
            capability, task, requirement
        );
        fs::write(definition.root_dir.join("main.rs"), code)?;

        // 4. Verification (Mock: In practice, run cargo check/test on the new skill)
        // If compilation fails, we would return error and not register.

        Ok(definition)
    }

    fn persist_definition(definition: &SkillDefinition) -> Result<()> {
        fs::create_dir_all(&definition.root_dir)?;
        fs::write(
            definition.root_dir.join("skill.json"),
            serde_json::to_vec_pretty(&json!({
                "name": definition.metadata.name,
                "version": definition.metadata.version,
                "capabilities": definition.metadata.capabilities,
                "entrypoint": definition.metadata.entrypoint,
                "permissions": definition.metadata.permissions,
            }))?,
        )?;
        Ok(())
    }
}
