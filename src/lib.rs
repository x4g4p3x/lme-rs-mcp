//! MCP transport adapter for the protocol-neutral [`lme_rs_agent`] crate.
//!
//! Statistical and session semantics live in [`lme_rs_agent::LmeAgentApi`].
//! [`LmeMcpServer`] only maps that API onto MCP tools and structured content.

mod mcp;
mod tasks;

pub use lme_rs_agent::*;
pub use mcp::LmeMcpServer;
