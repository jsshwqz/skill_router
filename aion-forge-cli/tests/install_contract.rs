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
fn installers_make_aion_forge_the_primary_command() {
    for path in ["scripts/install/install.ps1", "scripts/install/install.sh"] {
        let installer = read(path);
        assert!(installer.contains("aion-forge"), "missing canonical command in {path}");
        assert!(
            installer.contains("aion-forge-cli"),
            "missing compatibility command in {path}"
        );
        assert!(installer.contains("aion-forge acp"), "missing ACP example in {path}");
        assert!(
            installer.contains("aion-forge mcp-server"),
            "missing MCP example in {path}"
        );
    }
}
