use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
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
