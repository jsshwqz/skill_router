use std::{fs, path::PathBuf};

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CLI crate must be inside the workspace")
        .join(path)
}

fn read(path: &str) -> String {
    let file = workspace_file(path);
    fs::read_to_string(&file).unwrap_or_else(|error| panic!("failed to read {}: {error}", file.display()))
}

#[test]
fn release_publishes_canonical_and_compatibility_commands() {
    let workflow = read(".github/workflows/release.yml");
    assert!(workflow.contains("--bin aion-forge --bin aion-forge-cli"));
    assert!(workflow.contains("aion-forge-windows-x86_64.exe"));
    assert!(workflow.contains("aion-forge-cli-windows-x86_64.exe"));
}

#[test]
fn safety_manifest_tracks_both_command_names() {
    let manifest = read("safety-manifest.json");
    assert!(manifest.contains(r#""aion-forge": {"#));
    assert!(manifest.contains(r#""aion-forge-cli": {"#));
}
