use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use serde_json::Value;
use tracing_subscriber::EnvFilter;

use aion_types::types::RouterPaths;

mod catalog;
pub mod cli;
pub mod direct;
pub mod mcp;
pub mod setup;

/// Initialize and run the shared Aion Forge command-line entry point.
pub async fn main_entry() -> Result<()> {
    dotenvy::dotenv().ok();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let _ = dotenvy::from_path(directory.join(".env"));
        }
    }
    match std::env::var("AI_PROVIDERS_DISABLED") {
        Ok(value) if !value.trim().is_empty() => {}
        _ => unsafe { std::env::set_var("AI_PROVIDERS_DISABLED", "host-anthropic-proxy,ollama-local") },
    }

    let cli = cli::Cli::parse();
    let quiet = cli.quiet;
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(!quiet)
        .init();

    if let Some(result) = run_cli(cli).await? {
        let output = if quiet {
            serde_json::to_string(&result)?
        } else {
            serde_json::to_string_pretty(&result)?
        };
        println!("{output}");
    }
    Ok(())
}

/// Execute a parsed standalone CLI invocation.
pub async fn run_cli(cli: cli::Cli) -> Result<Option<Value>> {
    match cli.command {
        Some(cli::Commands::Acp) => {
            aion_forge_acp::run_acp_server().await?;
            Ok(None)
        }
        Some(cli::Commands::McpServer) => {
            unsafe { std::env::set_var("AION_MCP_MODE", "1") };
            let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            mcp::run(RouterPaths::for_workspace(&workspace)).await?;
            Ok(None)
        }
        Some(cli::Commands::Setup { dry_run }) => {
            let executable = std::env::current_exe()?;
            setup::run(dry_run, &executable).map(Some)
        }
        None if cli.list => Ok(Some(direct::list_tools())),
        None if cli.tool.is_some() || cli.params.is_some() || cli.quiet => {
            direct::execute(cli.tool.as_deref().unwrap_or(""), cli.params.as_deref(), cli.quiet)
                .await
                .map(Some)
        }
        None => {
            aion_forge_acp::run_acp_server().await?;
            Ok(None)
        }
    }
}
