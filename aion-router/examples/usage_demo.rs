use anyhow::Result;
use aion_router::SkillRouter;
use aion_types::types::RouterPaths;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🌟 Aion-Forge Integration Demo: Discovery + Automation");
    
    // 1. Setup a "Foreign Project" with a specific skill
    let temp_root = std::env::temp_dir().join("aion_integ_demo");
    if temp_root.exists() { fs::remove_dir_all(&temp_root)?; }
    fs::create_dir_all(&temp_root)?;
    
    let foreign_project = temp_root.join("external_security_app");
    let external_skills_dir = foreign_project.join(".aion/skills");
    fs::create_dir_all(&external_skills_dir)?;
    
    let skill_content = r#"{
        "metadata": { "name": "DeepSecurityScanner", "version": "1.0.0", "description": "Scan for vulnerabilities" },
        "capabilities": ["SecurityScan"],
        "source": { "Local": { "path": "deep_scan.sh" } }
    }"#;
    fs::write(external_skills_dir.join("scanner.json"), skill_content)?;
    
    // 2. Setup the "Local Workspace" which has no local security skills
    let local_ws = temp_root.join("main_app");
    let paths = RouterPaths::for_workspace(&local_ws);
    paths.ensure_base_dirs()?;
    
    // Inject foreign project into project.json for discovery
    let project_config = format!(r#"{{ "linked_projects": ["{}"] }}"#, foreign_project.display().to_string().replace("\\", "\\\\"));
    fs::write(local_ws.join("project.json"), project_config)?;
    
    println!("🚀 Starting Router in: {:?}", local_ws);
    let router = SkillRouter::new(paths.clone())?;
    
    // 3. Trigger Routing with a mission that needs the foreign skill
    println!("🔍 Mission: 'Perform a deep security scan on the source code'");
    let result = router.route("Perform a deep security scan on the source code").await?;
    
    println!("✅ INTEGRATION SUCCESS!");
    println!("📦 Found Skill: {} (from {})", result.skill.metadata.name, result.skill.metadata.version);
    println!("🛡️ Recovery Logic Active: {:?}", result.lifecycle);
    
    // Cleanup
    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}
