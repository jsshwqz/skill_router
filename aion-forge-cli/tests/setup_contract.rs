use std::path::Path;

#[test]
fn dry_run_config_points_to_cli_mcp_server() {
    let config = aion_forge_cli::setup::dry_run_config(Path::new(r"D:\tools\aion-forge-cli.exe"));
    assert_eq!(
        config["mcpServers"]["aion-forge"]["command"],
        r"D:\tools\aion-forge-cli.exe"
    );
    assert_eq!(
        config["mcpServers"]["aion-forge"]["args"],
        serde_json::json!(["mcp-server"])
    );
    assert!(config.to_string().find("API_KEY").is_none());
}

#[test]
fn setup_without_supported_helper_is_stable_error() {
    let error = aion_forge_cli::setup::run(false, Path::new("aion-forge-cli.exe"))
        .expect_err("unsupported helper context must not write configuration");
    assert!(error.to_string().contains("CONFIG_ENV_MISSING"));
}
