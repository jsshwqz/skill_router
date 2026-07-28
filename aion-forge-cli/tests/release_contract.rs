use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
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
