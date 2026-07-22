use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use aion_forge_cli::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let _ = dotenvy::from_path(directory.join(".env"));
        }
    }
    match std::env::var("AI_PROVIDERS_DISABLED") {
        Ok(value) if !value.trim().is_empty() => {}
        _ => std::env::set_var("AI_PROVIDERS_DISABLED", "host-anthropic-proxy,ollama-local"),
    }

    let cli = Cli::parse();
    let quiet = cli.quiet;
    let is_mcp = matches!(cli.command, Some(Commands::McpServer));
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(!cli.quiet)
        .init();

    if is_mcp {
        std::env::set_var("AION_MCP_MODE", "1");
    }

    if let Some(result) = aion_forge_cli::run_cli(cli).await? {
        let output = if quiet {
            serde_json::to_string(&result)?
        } else {
            serde_json::to_string_pretty(&result)?
        };
        println!("{output}");
    }
    Ok(())
}
