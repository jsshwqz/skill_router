//! Aion ZhanLue (战略) — Dialectical Strategy for AI Agent Orchestration
//!
//! Inspired by [dialectical-vibe-coding](https://github.com/whatwoods/dialectical-vibe-coding)
//! by @whatwoods, which introduced the Guide + Sensor harness model and
//! thesis-antithesis-synthesis workflow for AI agents.
//!
//! This crate extends those ideas with:
//! - Contradiction analysis (矛盾论) for bottleneck identification
//! - Three-phase strategic planning (论持久战) for complex tasks
//! - Dialectical retry with root-cause learning (实践论)
//! - Direct integration with aion-forge's SkillRouter and MemoryManager

mod ai;
mod engine;
mod dialectic;
mod contradiction;
mod strategy;
mod retry;
mod contract;
mod mcp;

use clap::{Parser, Subcommand};
use engine::Engine;
use std::io;

#[derive(Parser)]
#[command(name = "aion-zl", version, about = "Aion ZhanLue - Dialectical Strategy")]
struct Cli {
    #[arg(value_name = "TASK")]
    task: Option<String>,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start MCP server (stdio)
    McpServer,
    /// Export capability manifest
    ExportManifest,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aion_zl=info,aion_router=warn".into()),
        )
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Cmd::McpServer) => {
            let engine = Engine::new()?;
            mcp::run(&engine).await
        }
        Some(Cmd::ExportManifest) => {
            println!("{}", include_str!("../../manifest.json"));
            Ok(())
        }
        None => {
            if let Some(task) = cli.task {
                let engine = Engine::new()?;
                eprintln!("Aion ZhanLue v0.1.0");
                eprintln!("Task: {}", task);
                let result = engine.task_dialectic(&task).await?;
                println!("{}", serde_json::to_string_pretty(&result)?);
                Ok(())
            } else {
                eprintln!("Usage: aion-zl <TASK> | aion-zl mcp-server");
                std::process::exit(1);
            }
        }
    }
}
