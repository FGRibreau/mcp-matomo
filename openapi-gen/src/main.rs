mod client;
mod openapi;
mod parser;
mod schema_inference;
mod types;

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::client::MatomoClient;
use crate::openapi::generate_openapi;
use crate::parser::{convert_parameter, get_common_parameters, parse_api_reference, parse_method_list};
use crate::schema_inference::infer_schema;
use crate::types::{MatomoMethod, MatomoParameter};

#[derive(Parser, Debug)]
#[command(
    name = "matomo-openapi-generator",
    about = "Generate OpenAPI specification from Matomo API by introspection",
    version
)]
struct Args {
    /// Base URL of the Matomo instance (e.g., https://matomo.example.com)
    #[arg(short, long)]
    url: String,

    /// Authentication cookies (format: "cookie1=value1; cookie2=value2")
    #[arg(short, long, default_value = "")]
    cookies: String,

    /// Matomo API token (token_auth) - alternative to cookies
    #[arg(short = 't', long)]
    token: Option<String>,

    /// Output file path for the OpenAPI JSON specification
    #[arg(short, long, default_value = "matomo-openapi.json")]
    output: PathBuf,

    /// Delay between API requests in milliseconds (to avoid rate limiting)
    #[arg(short, long, default_value = "100")]
    delay: u64,

    /// Fetch example responses for each method (slower but provides better schema inference)
    #[arg(long, default_value = "false")]
    fetch_examples: bool,

    /// Site ID to use when fetching methods and examples
    #[arg(short = 's', long, default_value = "1")]
    site_id: String,

    /// Date to use when fetching examples
    #[arg(long, default_value = "yesterday")]
    date: String,

    /// Period to use when fetching examples
    #[arg(long, default_value = "day")]
    period: String,

    /// Maximum number of methods to process (0 = all)
    #[arg(long, default_value = "0")]
    limit: usize,

    /// Output intermediate results as well
    #[arg(long, default_value = "false")]
    verbose_output: bool,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    info!("Starting Matomo OpenAPI Generator");
    info!("Target URL: {}", args.url);

    // Create HTTP client
    let client = MatomoClient::new(&args.url, &args.cookies, args.token.as_deref())
        .context("Failed to create HTTP client")?;

    // Fetch Matomo version
    let version = client.fetch_version().unwrap_or_else(|_| "unknown".to_string());
    info!("Matomo version: {}", version);

    // Step 1: Fetch the list of all API methods using getReportMetadata
    info!("Fetching API method list for site {}...", args.site_id);
    let method_list_json = client.fetch_method_list(&args.site_id)?;

    if args.verbose_output {
        let methods_file = args.output.with_extension("methods.json");
        fs::write(
            &methods_file,
            serde_json::to_string_pretty(&method_list_json)?,
        )?;
        info!("Saved method list to {:?}", methods_file);
    }

    // Step 2: Parse the method list
    let methods = parse_method_list(&method_list_json)?;
    info!("Found {} API methods", methods.len());

    // Apply limit if specified
    let methods_to_process: Vec<_> = if args.limit > 0 {
        methods.into_iter().take(args.limit).collect()
    } else {
        methods
    };

    // Step 3: Fetch API reference page for parameter information
    info!("Fetching API reference documentation...");
    let api_reference = client.fetch_api_reference().unwrap_or_default();
    let method_metadata = parse_api_reference(&api_reference).unwrap_or_default();

    if args.verbose_output && !method_metadata.is_empty() {
        let metadata_file = args.output.with_extension("metadata.json");
        fs::write(
            &metadata_file,
            serde_json::to_string_pretty(&method_metadata)?,
        )?;
        info!("Saved method metadata to {:?}", metadata_file);
    }

    // Step 4: Build complete method definitions with examples
    let mut matomo_methods: Vec<MatomoMethod> = Vec::new();
    let total_methods = methods_to_process.len();
    let common_params = get_common_parameters();

    for (idx, parsed_method) in methods_to_process.iter().enumerate() {
        info!(
            "[{}/{}] Processing {}.{} ({})",
            idx + 1,
            total_methods,
            parsed_method.module,
            parsed_method.action,
            parsed_method.name
        );

        let method_name = format!("{}.{}", parsed_method.module, parsed_method.action);

        // Get parameters from metadata if available
        let mut parameters: Vec<MatomoParameter> = method_metadata
            .get(&method_name)
            .map(|m| m.parameters.iter().map(convert_parameter).collect())
            .unwrap_or_else(Vec::new);

        // Add common parameters if not already present
        for common_param in &common_params {
            if !parameters.iter().any(|p| p.name == common_param.name) {
                parameters.push(common_param.clone());
            }
        }

        // Fetch example response if requested
        let (example_response, response_schema) = if args.fetch_examples {
            thread::sleep(Duration::from_millis(args.delay));

            let example_params = [
                ("idSite", args.site_id.as_str()),
                ("date", args.date.as_str()),
                ("period", args.period.as_str()),
            ];

            match client.fetch_example(&parsed_method.module, &parsed_method.action, &example_params) {
                Ok(example) => {
                    let schema = if !example.is_null() {
                        Some(infer_schema(&example))
                    } else {
                        None
                    };
                    (Some(example), schema)
                }
                Err(e) => {
                    warn!("Failed to fetch example for {}.{}: {}", parsed_method.module, parsed_method.action, e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        matomo_methods.push(MatomoMethod {
            name: method_name,
            module: parsed_method.module.clone(),
            action: parsed_method.action.clone(),
            parameters,
            example_response,
            response_schema,
            description: parsed_method.documentation.clone(),
            category: parsed_method.category.clone(),
        });
    }

    info!("Processed {} methods", matomo_methods.len());

    if args.verbose_output {
        let methods_detailed = args.output.with_extension("methods-detailed.json");
        fs::write(
            &methods_detailed,
            serde_json::to_string_pretty(&matomo_methods)?,
        )?;
        info!("Saved detailed methods to {:?}", methods_detailed);
    }

    // Step 5: Generate OpenAPI specification
    info!("Generating OpenAPI specification...");
    let openapi_spec = generate_openapi(&matomo_methods, &args.url, &version);

    // Write the final OpenAPI spec
    let openapi_json = serde_json::to_string_pretty(&openapi_spec)?;
    fs::write(&args.output, &openapi_json)?;

    info!("OpenAPI specification saved to {:?}", args.output);
    info!(
        "Generated spec contains {} paths across {} modules",
        openapi_spec.paths.len(),
        openapi_spec.tags.as_ref().map(|t| t.len()).unwrap_or(0)
    );

    Ok(())
}
