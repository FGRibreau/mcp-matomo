mod matomo_client;
mod openapi;
mod service;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::openapi::OpenApiSpec;
use crate::service::MatomoService;

#[derive(Parser, Debug)]
#[command(
    name = "mcp-matomo",
    about = "MCP server for Matomo Analytics API",
    version
)]
struct Args {
    /// Path to the OpenAPI JSON specification file
    #[arg(short, long, env = "MCP_MATOMO_OPENAPI_FILE")]
    openapi: PathBuf,

    /// Matomo API token (token_auth)
    #[arg(short, long, env = "MCP_MATOMO_TOKEN")]
    token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (NEVER stdout for stdio transport!)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    info!("Starting MCP Matomo server");
    info!("Loading OpenAPI spec from: {:?}", args.openapi);

    // Load OpenAPI specification
    let spec = OpenApiSpec::from_file(
        args.openapi.to_str().context("Invalid path")?
    ).context("Failed to load OpenAPI specification")?;

    info!("Loaded OpenAPI spec: {} v{}", spec.info.title, spec.info.version);
    info!("Base URL: {:?}", spec.get_base_url());

    // Create the MCP service
    let service = MatomoService::new(spec, args.token)
        .context("Failed to create Matomo service")?;

    // Start the stdio transport
    info!("Starting stdio transport...");
    let server = service.serve(stdio()).await
        .context("Failed to start MCP server")?;

    // Wait for the server to complete
    server.waiting().await?;

    info!("MCP server stopped");
    Ok(())
}
