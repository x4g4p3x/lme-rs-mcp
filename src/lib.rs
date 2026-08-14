//! Agent-facing adapter layer for [`lme-rs`](https://github.com/x4g4p3x/lme-rs).
//!
//! Statistical and session semantics live in [`LmeAgentApi`]. [`LmeMcpServer`] is a thin
//! MCP transport adapter that exposes the same typed request/response model as structured
//! MCP tool content. Keeping the core protocol-neutral makes it reusable by future
//! scientific protocol adapters without duplicating model logic.

mod api;
mod data;
mod dto;
mod mcp;
mod session;

pub use api::{AgentApiError, LmeAgentApi};
pub use data::load_csv;
pub use dto::{
    AnovaRequest, AnovaRow, AnovaSummary, BootConfintRow, BootSummary, BootstrapRequest,
    FitIdRequest, FitListEntry, FitListSummary, FitLmmRequest, FitModelRequest, FitSummary,
    ForgetFitResult, ModelKind,
};
pub use mcp::LmeMcpServer;
pub use session::{CachedFit, FitSession};
