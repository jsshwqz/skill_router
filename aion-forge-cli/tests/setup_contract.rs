use std::path::Path;

use serde_json::json;

#[test]
fn dry_run_config_points_to_cli_mcp_server() {
    let config = aion_forge_cli::setup::dry_run_config(Path::new(r"D:\tools\aion-forge-cli.exe"));
    assert_eq!(
        config["mcpServers"]["aion-forge"]["command"],
        r"D:\tools\aion-forge.exe"
    );
    assert_eq!(
        config["mcpServers"]["aion-forge"]["args"],
        serde_json::json!(["mcp-server"])
    );
    assert!(config.to_string().find("API_KEY").is_none());
}

#[test]
fn setup_without_supported_helper_is_stable_error() {
    let previous = std::env::var_os("AIONUI_HELPER_BIN");
    std::env::remove_var("AIONUI_HELPER_BIN");
    let error = aion_forge_cli::setup::run(false, Path::new("aion-forge-cli.exe"))
        .expect_err("unsupported helper context must not write configuration");
    if let Some(previous) = previous {
        std::env::set_var("AIONUI_HELPER_BIN", previous);
    }
    assert!(error.to_string().contains("CONFIG_ENV_MISSING"));
}

#[test]
fn update_input_targets_existing_forge_server() {
    let listed = json!({
        "success": true,
        "data": {
            "servers": [{
                "server_id": "mcp_forge",
                "name": "aion-forge"
            }]
        }
    });
    let env = json!({"AI_MODEL": "auto/fast"});

    let input = aion_forge_cli::setup::build_update_input(&listed, Path::new(r"D:\tools\aion-forge-cli.exe"), &env)
        .expect("existing Forge server should be found");

    assert_eq!(input["server_id"], "mcp_forge");
    assert_eq!(input["transport"]["command"], r"D:\tools\aion-forge.exe");
    assert_eq!(input["transport"]["args"], json!(["mcp-server"]));
    assert_eq!(input["transport"]["env"], env);
}

#[test]
fn update_input_reuses_legacy_named_server() {
    let listed = json!({
        "data": {
            "servers": [{
                "id": "legacy_forge",
                "name": "aion-forge-cli"
            }]
        }
    });

    let input =
        aion_forge_cli::setup::build_update_input(&listed, Path::new(r"D:\tools\aion-forge-cli.exe"), &json!({}))
            .expect("legacy Forge registration should be reused");

    assert_eq!(input["server_id"], "legacy_forge");
    assert_eq!(input["transport"]["command"], r"D:\tools\aion-forge.exe");
}
