use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::Parser;
use ghost_fl_mcp::{FlMcpServer, MCP_PROTOCOL_VERSION};
use ghost_fl_studio::{FlStudioAdapterConfig, GopherNativeAdapter, DEFAULT_DEBUG_PORT};
use rmcp::{transport::stdio, ServiceExt};

#[derive(Debug, Parser)]
#[command(
    name = "ghost-fl-mcp",
    about = "MCP 2026-07-28 stdio export of the raw live FL Studio/Gopher tool catalog"
)]
struct Cli {
    #[arg(long, default_value_t = DEFAULT_DEBUG_PORT)]
    debug_port: u16,

    #[arg(long, default_value = "gopher")]
    target_match: String,

    #[arg(long = "i-accept-live-fl-writes")]
    i_accept_live_fl_writes: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if !cli.i_accept_live_fl_writes {
        bail!(
            "ghost-fl-mcp exposes the complete live Gopher catalog, including destructive tools; pass --i-accept-live-fl-writes only after opening a project you are willing to modify"
        );
    }

    let adapter = Arc::new(
        GopherNativeAdapter::connect(FlStudioAdapterConfig {
            debug_port: cli.debug_port,
            target_match: cli.target_match,
            ..Default::default()
        })
        .context("failed to connect to the live FL Studio Gopher target")?,
    );
    let manifest = adapter
        .manifest()
        .context("failed to read the live FL Studio Gopher manifest")?;
    let tool_count = manifest.tools.len();
    let target_title = manifest.target_title.clone();
    let server = FlMcpServer::from_gopher(&manifest, adapter)
        .context("failed to convert the live Gopher manifest into MCP tools")?;

    eprintln!("[ghost-fl-mcp] connected to '{target_title}' with {tool_count} raw Gopher tools");
    eprintln!(
        "[ghost-fl-mcp] serving MCP {MCP_PROTOCOL_VERSION} over stdio; stdout is reserved for protocol traffic"
    );

    let service = server
        .serve(stdio())
        .await
        .context("failed to start MCP stdio service")?;
    service
        .waiting()
        .await
        .context("MCP stdio service terminated with an error")?;
    Ok(())
}
