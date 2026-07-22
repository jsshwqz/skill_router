use clap::{CommandFactory, Parser};

use aion_forge_cli::cli::{Cli, Commands};

#[test]
fn cli_keeps_direct_flags_and_adds_mcp_without_acp() {
    let direct = Cli::try_parse_from([
        "aion-forge-cli",
        "--tool",
        "text_summarize",
        "--params",
        r#"{"text":"hello"}"#,
        "--quiet",
    ])
    .expect("direct invocation should parse");
    assert_eq!(direct.tool.as_deref(), Some("text_summarize"));
    assert!(direct.quiet);

    let mcp = Cli::try_parse_from(["aion-forge-cli", "mcp-server"]).expect("mcp-server should parse");
    assert!(matches!(mcp.command, Some(Commands::McpServer)));

    assert!(Cli::try_parse_from(["aion-forge-cli", "acp"]).is_err());
    Cli::command().debug_assert();
}

#[test]
fn setup_dry_run_is_supported() {
    let cli = Cli::try_parse_from(["aion-forge-cli", "setup", "--dry-run"]).expect("setup --dry-run should parse");
    assert!(matches!(cli.command, Some(Commands::Setup { dry_run: true })));
}
