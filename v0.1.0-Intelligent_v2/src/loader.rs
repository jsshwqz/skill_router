use std::path::Path;
use std::fs;
use anyhow::Result;
use crate::models::SkillMetadata;

pub struct Loader;

impl Loader {
    pub fn load_skills<P: AsRef<Path>>(skills_dir: P) -> Result<Vec<SkillMetadata>> {
        let mut skills = Vec::new();
        if !skills_dir.as_ref().exists() {
            fs::create_dir_all(&skills_dir)?;
            return Ok(skills);
        }

        for entry in fs::read_dir(skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_json_path = path.join("skill.json");
                if skill_json_path.exists() {
                    let content = fs::read_to_string(skill_json_path)?;
                    let skill_meta: SkillMetadata = serde_json::from_str(&content)?;
                    skills.push(skill_meta);
                }
            }
        }
        Ok(skills)
    }
}
