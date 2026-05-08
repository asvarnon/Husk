//! # Homelab MCP
//!
//! MCP adapter for Claude.

use anyhow::Result;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting homelab-mcp server...");

    // In a real implementation, we would start the MCP server here
    // and route calls to homelab_core and homelab_ops.

    info!("Server is running (placeholder). Press Ctrl+C to exit.");

    // Keep the process alive for the placeholder
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");
    Ok(())
}
