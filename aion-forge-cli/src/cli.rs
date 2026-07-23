use clap::{Parser, Subcommand};

/// Command-line arguments for the standalone Aion Forge entrypoint.
#[derive(Debug, Parser)]
#[command(name = "aion-forge", version, about = "Aion Forge agent, CLI, and MCP server")]
pub struct Cli {
    /// Run a protocol or setup command.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Execute one built-in tool directly.
    #[arg(short, long)]
    pub tool: Option<String>,

    /// JSON parameters for direct tool execution.
    #[arg(short, long)]
    pub params: Option<String>,

    /// List all built-in tools.
    #[arg(short, long)]
    pub list: bool,

    /// Emit compact result JSON.
    #[arg(short, long)]
    pub quiet: bool,
}

/// Standalone CLI subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the ACP JSON-RPC stdio server.
    Acp,

    /// Start the MCP JSON-RPC stdio server.
    McpServer,

    /// Prepare the AionUI MCP configuration.
    Setup {
        /// Print the generated configuration without persisting it.
        #[arg(short, long)]
        dry_run: bool,
    },
}
