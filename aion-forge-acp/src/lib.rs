//! Reusable ACP protocol entry point for Aion Forge.

mod acp;
pub mod catalog;
pub mod executor;
pub mod model_catalog;
pub mod session;

/// Run the ACP JSON-RPC server over stdin and stdout.
pub use acp::run_acp_server;
