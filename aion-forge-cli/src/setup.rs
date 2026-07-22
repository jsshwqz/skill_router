use std::path::Path;

use anyhow::{bail, Result};
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

/// Run setup without accessing AionUI's background database.
pub fn run(dry_run: bool, executable: &Path) -> Result<Value> {
    if dry_run {
        return Ok(dry_run_config(executable));
    }

    bail!("CONFIG_ENV_MISSING: supported AionUI helper context is unavailable")
}
