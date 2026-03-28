use aion_router::automation::discovery::{DiscoveryRadar, DiscoveryLayer};
use aion_types::types::RouterPaths;
use serde_json::json;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_local_skill_discovery() -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
    let test_dir_name = format!("aion_discovery_test_{}", now);
    let workspace_root = std::env::temp_dir().join(test_dir_name);
    fs::create_dir_all(&workspace_root)?;
    let paths = RouterPaths::for_workspace(&workspace_root);

    // Create a mock skill directory
    let skill_dir = paths.skills_dir.join("test-skill");
    fs::create_dir_all(&skill_dir)?;

    let skill_json = json!({
        "name": "test-skill",
        "version": "1.0.0",
        "capabilities": ["web_search", "summarize"],
        "entrypoint": "main.js"
    });
    fs::write(skill_dir.join("skill.json"), serde_json::to_string(&skill_json)?)?;

    let radar = DiscoveryRadar::new(paths);

    // Test positive match
    let result = radar.cascade_search("web_search")?;
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.layer, DiscoveryLayer::Local);
    assert_eq!(m.skill.metadata.name, "test-skill");
    assert!(m.skill.supports_capability("web_search"));

    // Test negative match
    let result_none = radar.cascade_search("non_existent")?;
    assert!(result_none.is_none());

    // Cleanup
    let _ = fs::remove_dir_all(&workspace_root);
    Ok(())
}

#[test]
fn test_project_skill_discovery() -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
    let workspace_root = std::env::temp_dir().join(format!("aion_ws_{}", now));
    let external_project = std::env::temp_dir().join(format!("aion_ext_{}", now));
    fs::create_dir_all(&workspace_root)?;
    fs::create_dir_all(&external_project)?;

    let paths = RouterPaths::for_workspace(&workspace_root);

    // Create a skill in the EXTERNAL project directory
    let ext_skill_dir = external_project.join("shared-skill");
    fs::create_dir_all(&ext_skill_dir)?;
    let skill_json = json!({
        "name": "shared-skill",
        "version": "1.0.0",
        "capabilities": ["shared_utils"],
        "entrypoint": "main.js"
    });
    fs::write(ext_skill_dir.join("skill.json"), serde_json::to_string(&skill_json)?)?;

    // Radar with project paths
    let radar = DiscoveryRadar::new(paths)
        .with_project_paths(vec![external_project.clone()]);

    // Test Project Layer match
    let result = radar.cascade_search("shared_utils")?;
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.layer, DiscoveryLayer::Project);
    assert_eq!(m.skill.metadata.name, "shared-skill");

    // Cleanup
    let _ = fs::remove_dir_all(&workspace_root);
    let _ = fs::remove_dir_all(&external_project);

    Ok(())
}

#[test]
fn test_search_by_payload() -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
    let workspace_root = std::env::temp_dir().join(format!("aion_payload_ws_{}", now));
    fs::create_dir_all(&workspace_root)?;

    let paths = RouterPaths::for_workspace(&workspace_root);
    let radar = DiscoveryRadar::new(paths);

    // Create a payload with an intent that exists in Central (Mocked)
    let payload = aion_types::ai_native::AiNativePayload::new("advanced_reasoning");
    
    let result = radar.search_by_payload(&payload)?;
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.layer, DiscoveryLayer::Central);
    assert_eq!(m.skill.metadata.name, "remote-advanced_reasoning");

    // Cleanup
    let _ = fs::remove_dir_all(&workspace_root);

    Ok(())
}

#[test]
fn test_central_skill_discovery() -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
    let workspace_root = std::env::temp_dir().join(format!("aion_ws_central_{}", now));
    fs::create_dir_all(&workspace_root)?;

    let paths = RouterPaths::for_workspace(&workspace_root);
    let radar = DiscoveryRadar::new(paths);

    // Test Central Layer match (Mocked)
    let result = radar.cascade_search("advanced_reasoning")?;
    assert!(result.is_some());
    let m = result.unwrap();
    assert_eq!(m.layer, DiscoveryLayer::Central);
    assert_eq!(m.skill.metadata.name, "remote-advanced_reasoning");
    assert_eq!(m.skill.source, aion_types::types::SkillSource::RemoteCandidate);

    // Cleanup
    let _ = fs::remove_dir_all(&workspace_root);

    Ok(())
}
