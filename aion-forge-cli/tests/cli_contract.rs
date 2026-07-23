use clap::{CommandFactory, Parser};

use aion_forge_cli::cli::{Cli, Commands};

#[test]
fn cli_keeps_direct_flags_and_supports_protocol_subcommands() {
    let direct = Cli::try_parse_from([
        "aion-forge",
        "--tool",
        "text_summarize",
        "--params",
        r#"{"text":"hello"}"#,
        "--quiet",
    ])
    .expect("direct invocation should parse");
    assert_eq!(direct.tool.as_deref(), Some("text_summarize"));
    assert!(direct.quiet);

    let mcp = Cli::try_parse_from(["aion-forge", "mcp-server"]).expect("mcp-server should parse");
    assert!(matches!(mcp.command, Some(Commands::McpServer)));

    let acp = Cli::try_parse_from(["aion-forge", "acp"]).expect("acp should parse");
    assert!(matches!(acp.command, Some(Commands::Acp)));
    Cli::command().debug_assert();
}

#[test]
fn setup_dry_run_is_supported() {
    let cli = Cli::try_parse_from(["aion-forge", "setup", "--dry-run"]).expect("setup --dry-run should parse");
    assert!(matches!(cli.command, Some(Commands::Setup { dry_run: true })));
}
