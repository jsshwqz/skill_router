//! Aion Forge ACP adapter.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod acp;

#[derive(Parser)]
#[command(name = "aion-forge-acp", version = "0.7.0", about = "Aion Forge ACP adapter")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the ACP protocol server over stdin/stdout.
    Acp,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    dotenvy::dotenv().ok();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = dotenvy::from_path(dir.join(".env"));
        }
    }

    match std::env::var("AI_PROVIDERS_DISABLED") {
        Ok(value) if !value.trim().is_empty() => {}
        _ => std::env::set_var("AI_PROVIDERS_DISABLED", "host-anthropic-proxy,ollama-local"),
    }

    aion_router::learner::init_learner(&std::env::current_dir().unwrap_or_default());

    match cli.command {
        Commands::Acp => acp::run_acp_server().await,
    }
}
