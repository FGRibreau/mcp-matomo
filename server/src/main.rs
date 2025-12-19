mod generator;
mod matomo_client;
mod openapi;
mod parser;
mod schema_inference;
mod service;
mod types;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::generator::{generate_openapi_spec, GeneratorConfig};
use crate::openapi::OpenApiSpec;
use crate::service::MatomoService;

#[derive(Parser, Debug)]
#[command(
    name = "mcp-matomo",
    about = "MCP server for Matomo Analytics API",
    long_about = "MCP server that dynamically introspects your Matomo instance and exposes all API methods as tools.\n\n\
                  The server can either:\n\
                  1. Generate the OpenAPI spec at startup by introspecting your Matomo instance (--url)\n\
                  2. Load a pre-generated OpenAPI spec from a file (--openapi)",
    version
)]
struct Args {
    /// Matomo instance URL (e.g., https://matomo.example.com)
    /// When provided, the server will introspect the Matomo API at startup
    #[arg(short, long, env = "MCP_MATOMO_URL", group = "source")]
    url: Option<String>,

    /// Path to a pre-generated OpenAPI JSON specification file
    /// Use this for faster startup if you have a cached spec
    #[arg(short, long, env = "MCP_MATOMO_OPENAPI_FILE", group = "source")]
    openapi: Option<PathBuf>,

    /// Matomo API token (token_auth)
    /// Required for accessing protected API methods
    #[arg(short, long, env = "MCP_MATOMO_TOKEN")]
    token: Option<String>,

    /// Site ID to use when introspecting the API (default: 1)
    #[arg(short, long, env = "MCP_MATOMO_SITE_ID", default_value = "1")]
    site_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (NEVER stdout for stdio transport!)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    info!("Starting MCP Matomo server");

    // Determine how to get the OpenAPI spec
    let spec = if let Some(url) = &args.url {
        // Generate spec by introspecting Matomo instance
        info!("Introspecting Matomo instance at: {}", url);
        let config = GeneratorConfig::new(url.clone(), args.token.clone())
            .with_site_id(args.site_id.clone());
        generate_openapi_spec(&config)
            .await
            .context("Failed to generate OpenAPI specification from Matomo instance")?
    } else if let Some(openapi_path) = &args.openapi {
        // Load spec from file
        info!("Loading OpenAPI spec from: {:?}", openapi_path);
        OpenApiSpec::from_file(openapi_path.to_str().context("Invalid path")?)
            .context("Failed to load OpenAPI specification")?
    } else {
        // Neither --url nor --openapi provided
        anyhow::bail!(
            "Either --url or --openapi must be provided.\n\n\
             Examples:\n\
             \n\
             1. Introspect Matomo at startup:\n\
                mcp-matomo --url https://matomo.example.com --token YOUR_TOKEN\n\
             \n\
             2. Use a pre-generated OpenAPI spec:\n\
                mcp-matomo --openapi matomo-api.json --token YOUR_TOKEN"
        );
    };

    info!(
        "Loaded OpenAPI spec: {} v{}",
        spec.info.title, spec.info.version
    );
    info!("Base URL: {:?}", spec.get_base_url());

    // Create the MCP service
    let service =
        MatomoService::new(spec, args.token).context("Failed to create Matomo service")?;

    // Start the stdio transport
    info!("Starting stdio transport...");
    let server = service
        .serve(stdio())
        .await
        .context("Failed to start MCP server")?;

    // Wait for the server to complete
    server.waiting().await?;

    info!("MCP server stopped");
    Ok(())
}
