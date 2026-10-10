//! Binary entry point for the `lme-rs` MCP server (stdio transport).

use anyhow::Result;
use lme_rs_mcp::LmeMcpServer;
use rmcp::{transport, ServiceExt};

#[tokio::main]
async fn main() -> Result<()> {
    let server = LmeMcpServer::new();
    let service = server.clone().serve(transport::stdio()).await?;
    service.waiting().await?;
    server.shutdown();
    Ok(())
}
