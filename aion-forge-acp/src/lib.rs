//! Reusable ACP protocol entry point for Aion Forge.

mod acp;

/// Run the ACP JSON-RPC server over stdin and stdout.
pub use acp::run_acp_server;
