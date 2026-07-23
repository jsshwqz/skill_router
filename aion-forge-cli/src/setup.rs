use std::{
    env,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// Build the redacted MCP configuration displayed by `setup --dry-run`.
pub fn dry_run_config(executable: &Path) -> Value {
    json!({
        "mcpServers": {
            "aion-forge": {
                "command": executable,
                "args": ["mcp-server"]
            }
        }
    })
}

/// Build the AionUI MCP update input for the existing Forge server.
pub fn build_update_input(listed: &Value, executable: &Path, env: &Value) -> Result<Value> {
    let servers = listed
        .pointer("/data/servers")
        .or_else(|| listed.get("data"))
        .and_then(Value::as_array)
        .context("CONFIG_MCP_NOT_FOUND: aion-forge server is not registered")?;
    let server_id = servers
        .iter()
        .find(|server| server.get("name").and_then(Value::as_str) == Some("aion-forge"))
        .and_then(|server| {
            server
                .get("server_id")
                .or_else(|| server.get("id"))
                .and_then(Value::as_str)
        })
        .context("CONFIG_MCP_NOT_FOUND: aion-forge server is not registered")?;

    Ok(json!({
        "server_id": server_id,
        "transport": {
            "type": "stdio",
            "command": executable,
            "args": ["mcp-server"],
            "env": env
        }
    }))
}

fn invoke_helper(helper: &Path, args: &[&str], input: Option<&Value>) -> Result<Value> {
    let mut child = Command::new(helper)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("CONFIG_HELPER_FAILED: unable to start AionUI helper")?;
    if let Some(input) = input {
        serde_json::to_writer(
            child
                .stdin
                .as_mut()
                .context("CONFIG_HELPER_FAILED: helper stdin unavailable")?,
            input,
        )?;
    }
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stable_error = String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("CONFIG_HELPER_FAILED")
            .trim()
            .to_owned();
        bail!(stable_error);
    }
    serde_json::from_slice(&output.stdout).context("CONFIG_HELPER_FAILED: helper returned invalid JSON")
}

fn configured_env(executable: &Path) -> Value {
    executable
        .parent()
        .map(|parent| parent.join(".mcp.json"))
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|config| config.pointer("/mcpServers/aion-forge/env").cloned())
        .unwrap_or_else(|| json!({}))
}

/// Persist the Forge MCP entry through AionUI's supported config helper.
pub fn run(dry_run: bool, executable: &Path) -> Result<Value> {
    if dry_run {
        return Ok(dry_run_config(executable));
    }

    let helper = env::var_os("AIONUI_HELPER_BIN")
        .filter(|value| !value.is_empty())
        .context("CONFIG_ENV_MISSING: supported AionUI helper context is unavailable")?;
    let helper = Path::new(&helper);
    let listed = invoke_helper(helper, &["config", "mcp", "servers", "list"], None)?;
    let input = build_update_input(&listed, executable, &configured_env(executable))?;
    invoke_helper(helper, &["config", "mcp", "servers", "update"], Some(&input))?;
    let read_back = invoke_helper(helper, &["config", "mcp", "servers", "list"], None)?;
    build_update_input(&read_back, executable, &json!({}))?;

    Ok(json!({
        "configured": true,
        "server": "aion-forge",
        "command": executable,
        "args": ["mcp-server"]
    }))
}
