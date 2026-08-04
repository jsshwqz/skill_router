use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

#[test]
fn list_catalog_matches_the_77_tool_product_contract() {
    let catalog = aion_forge_cli::direct::list_tools();
    let tools = catalog["tools"].as_array().expect("tools must be an array");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name must be a string"))
        .collect();
    let unique: HashSet<&str> = names.iter().copied().collect();
    let builtins = aion_router::builtins::BuiltinRegistry::default_registry();
    let capabilities = aion_types::capability_registry::CapabilityRegistry::builtin();
    let declared: HashSet<&str> = capabilities
        .definitions()
        .map(|definition| definition.name.as_str())
        .collect();
    let missing_declarations: Vec<&str> = builtins
        .list_skills()
        .into_iter()
        .filter(|name| *name != "ai_task")
        .filter(|name| !declared.contains(name))
        .collect();
    let routable: HashSet<&str> = builtins.list_skills().into_iter().collect();
    let undeclared_routes: Vec<&str> = declared
        .iter()
        .copied()
        .filter(|name| !routable.contains(name))
        .collect();

    assert_eq!(catalog["total"], 77);
    assert_eq!(names.len(), 77);
    assert_eq!(
        unique.len(),
        77,
        "direct catalog contains duplicate names; routable skills missing declarations: {missing_declarations:?}; declarations without routes: {undeclared_routes:?}"
    );
    assert!(unique.contains("sanitize"), "sanitize is the missing public capability");
    assert!(
        !unique.contains("ai_task"),
        "ai_task is an internal dispatcher, not a public capability"
    );
    assert!(
        builtins.get("sanitize").is_some(),
        "the added public capability must have a builtin route"
    );
    assert!(
        missing_declarations.is_empty(),
        "builtin routes missing public declarations: {missing_declarations:?}"
    );
    assert!(
        undeclared_routes.is_empty(),
        "public declarations without builtin routes: {undeclared_routes:?}"
    );
    assert!(
        names
            .iter()
            .all(|name| capabilities.get(name).is_some() || *name == "sanitize")
    );

    let acp_catalog = aion_forge_acp::catalog::CapabilityCatalog::live();
    let acp_names: HashSet<&str> = acp_catalog.entries().iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(unique, acp_names, "direct and ACP catalogs must expose the same names");
}

#[tokio::test]
async fn execute_runs_echo_from_builtin_registry() {
    let result = aion_forge_cli::direct::execute("echo", Some(r#"{"text":"forge-direct-contract"}"#), true)
        .await
        .expect("echo should execute through the built-in registry");

    assert_eq!(result["echo"], "forge-direct-contract");
    assert_eq!(result["capability"], "echo");
    assert_eq!(result["length"], 21);
}

#[tokio::test]
async fn health_check_defaults_to_zero_request_historical_mode() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root should exist")
        .join("target")
        .join("health-contract");
    let params = serde_json::json!({
        "workspace": workspace,
        "live_probe": false,
    });
    let result = aion_forge_cli::direct::execute("health_check", Some(&params.to_string()), true)
        .await
        .expect("health_check should execute through the built-in registry");

    assert_eq!(result["live_probe_performed"], false);
    assert!(result["live_probes"].is_null());
    assert_eq!(result["server_version"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn quiet_direct_output_is_one_compact_json_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_aion-forge-cli"))
        .args(["--tool", "echo", "--params", r#"{"text":"quiet-contract"}"#, "--quiet"])
        .output()
        .expect("quiet direct invocation should start");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(stdout.lines().count(), 1, "quiet output must be one JSON line");
    let value: serde_json::Value = serde_json::from_str(stdout.trim_end()).expect("quiet output should be valid JSON");
    assert_eq!(value["capability"], "echo");
    assert_eq!(value["echo"], "quiet-contract");
    assert_eq!(value["length"], 14);
}

#[test]
fn direct_session_report_initializes_learning_engine() {
    let learning_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root should exist")
        .join("target")
        .join("direct-learning-contract");
    let output = Command::new(env!("CARGO_BIN_EXE_aion-forge-cli"))
        .env("AION_LEARNING_DIR", learning_dir)
        .args(["--tool", "session_report", "--params", "{}", "--quiet"])
        .output()
        .expect("direct session_report invocation should start");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_ne!(result["error"], "学习引擎未初始化");
    assert!(result.get("session").is_some() || result.get("summary").is_some());
}
