//! Reusable ACP protocol entry point for Aion Forge.

mod acp;
pub mod agent_loop;
pub mod catalog;
pub mod executor;
pub mod model_catalog;
pub mod planner;
pub mod session;

/// Run the ACP JSON-RPC server over stdin and stdout.
pub use acp::run_acp_server;
