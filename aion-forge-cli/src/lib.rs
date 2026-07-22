use std::path::PathBuf;

use anyhow::Result;
use serde_json::Value;

use aion_types::types::RouterPaths;

pub mod cli;
pub mod direct;
pub mod mcp;
pub mod setup;

/// Execute a parsed standalone CLI invocation.
pub async fn run_cli(cli: cli::Cli) -> Result<Option<Value>> {
    match cli.command {
        Some(cli::Commands::McpServer) => {
            std::env::set_var("AION_MCP_MODE", "1");
            let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            mcp::run(RouterPaths::for_workspace(&workspace)).await?;
            Ok(None)
        }
        Some(cli::Commands::Setup { dry_run }) => {
            let executable = std::env::current_exe()?;
            setup::run(dry_run, &executable).map(Some)
        }
        None if cli.list => Ok(Some(direct::list_tools())),
        None => direct::execute(cli.tool.as_deref().unwrap_or(""), cli.params.as_deref(), cli.quiet)
            .await
            .map(Some),
    }
}
