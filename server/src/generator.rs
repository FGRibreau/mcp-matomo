//! OpenAPI specification generator that introspects Matomo API at runtime
//!
//! This module provides async functions to fetch Matomo API metadata and
//! generate an OpenAPI specification dynamically at server startup.

use anyhow::{Context, Result};
use indexmap::IndexMap;
use reqwest::Client;
use std::collections::HashMap;
use tracing::{info, warn};
use url::Url;

use crate::openapi::{
    Components, Info, OpenApiSpec, Operation, Parameter, ParameterSchema, PathItem, Response,
    SecurityScheme, Server, Tag,
};
use crate::parser::{
    convert_parameter, get_common_parameters, parse_api_reference, parse_method_list,
};
use crate::types::{JsonSchema, MatomoMethod, MatomoParameter};

/// Configuration for OpenAPI generation
pub struct GeneratorConfig {
    pub base_url: String,
    pub token: Option<String>,
    pub site_id: String,
}

impl GeneratorConfig {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        Self {
            base_url,
            token,
            site_id: "1".to_string(),
        }
    }

    pub fn with_site_id(mut self, site_id: String) -> Self {
        self.site_id = site_id;
        self
    }
}

/// Async Matomo client for introspection
struct IntrospectionClient {
    client: Client,
    base_url: Url,
    token_auth: Option<String>,
}

impl IntrospectionClient {
    fn new(base_url: &str, token: Option<String>) -> Result<Self> {
        let base_url = Url::parse(base_url).context("Invalid base URL")?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .danger_accept_invalid_certs(true) // Some Matomo instances have self-signed certs
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            base_url,
            token_auth: token,
        })
    }

    /// Make an API request - uses POST when token is present
    async fn api_request(
        &self,
        module: &str,
        action: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<String> {
        let mut url = self.base_url.clone();
        url.set_path("index.php");

        let method_str = format!("{}.{}", module, action);

        if let Some(ref token) = self.token_auth {
            // Use POST with form data when token is present
            let mut form_params: Vec<(&str, &str)> = vec![
                ("module", "API"),
                ("method", &method_str),
                ("format", "JSON"),
                ("token_auth", token),
            ];

            for (key, value) in extra_params {
                form_params.push((key, value));
            }

            let response = self
                .client
                .post(url.as_str())
                .form(&form_params)
                .send()
                .await
                .context("Failed to send POST request")?;

            let status = response.status();
            let text = response.text().await.context("Failed to read response")?;

            if !status.is_success() {
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    anyhow::bail!(
                        "Authentication failed (HTTP 401). Please check your API token.\n\
                         Response: {}",
                        text
                    );
                }
                anyhow::bail!("HTTP error {}: {}", status, text);
            }

            Ok(text)
        } else {
            // Use GET when no token
            {
                let mut query = url.query_pairs_mut();
                query.clear();
                query.append_pair("module", "API");
                query.append_pair("method", &method_str);
                query.append_pair("format", "JSON");

                for (key, value) in extra_params {
                    query.append_pair(key, value);
                }
            }

            let response = self
                .client
                .get(url.as_str())
                .send()
                .await
                .context("Failed to send GET request")?;

            let status = response.status();
            let text = response.text().await.context("Failed to read response")?;

            if !status.is_success() {
                anyhow::bail!("HTTP error {}: {}", status, text);
            }

            Ok(text)
        }
    }

    /// Fetch Matomo version
    async fn fetch_version(&self) -> Result<String> {
        let text = self.api_request("API", "getMatomoVersion", &[]).await?;
        let json: serde_json::Value =
            serde_json::from_str(&text).context("Failed to parse version JSON")?;
        Ok(json
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    /// Fetch method list using getReportMetadata
    async fn fetch_method_list(&self, site_id: &str) -> Result<serde_json::Value> {
        let extra_params = [("idSite", site_id)];
        let text = self
            .api_request("API", "getReportMetadata", &extra_params)
            .await?;
        serde_json::from_str(&text).context("Failed to parse method list JSON")
    }

    /// Fetch API reference HTML
    async fn fetch_api_reference(&self) -> Result<String> {
        self.api_request("API", "listAllAPI", &[]).await
    }
}

/// Generate OpenAPI specification by introspecting a Matomo instance
pub async fn generate_openapi_spec(config: &GeneratorConfig) -> Result<OpenApiSpec> {
    info!("Generating OpenAPI specification from Matomo instance...");
    info!("Target URL: {}", config.base_url);

    let client = IntrospectionClient::new(&config.base_url, config.token.clone())?;

    // Fetch Matomo version
    let version = client.fetch_version().await.unwrap_or_else(|e| {
        warn!("Could not fetch Matomo version: {}", e);
        "unknown".to_string()
    });
    info!("Matomo version: {}", version);

    // Fetch method list
    info!("Fetching API method list for site {}...", config.site_id);
    let method_list_json = client.fetch_method_list(&config.site_id).await?;
    let parsed_methods = parse_method_list(&method_list_json)?;
    info!("Found {} API methods", parsed_methods.len());

    // Fetch API reference for parameter info
    info!("Fetching API reference documentation...");
    let api_reference = client.fetch_api_reference().await.unwrap_or_default();
    let method_metadata = parse_api_reference(&api_reference).unwrap_or_default();

    // Build complete method definitions
    let common_params = get_common_parameters();
    let mut matomo_methods: Vec<MatomoMethod> = Vec::new();

    for parsed_method in &parsed_methods {
        let method_name = format!("{}.{}", parsed_method.module, parsed_method.action);

        // Get parameters from metadata if available
        let mut parameters: Vec<MatomoParameter> = method_metadata
            .get(&method_name)
            .map(|m| m.parameters.iter().map(convert_parameter).collect())
            .unwrap_or_default();

        // Add common parameters if not already present
        for common_param in &common_params {
            if !parameters.iter().any(|p| p.name == common_param.name) {
                parameters.push(common_param.clone());
            }
        }

        matomo_methods.push(MatomoMethod {
            name: method_name,
            module: parsed_method.module.clone(),
            action: parsed_method.action.clone(),
            parameters,
            example_response: None,
            response_schema: None,
            description: parsed_method.documentation.clone(),
            category: parsed_method.category.clone(),
        });
    }

    info!("Processed {} methods", matomo_methods.len());

    // Generate OpenAPI specification
    let spec = build_openapi_spec(&matomo_methods, &config.base_url, &version);

    info!(
        "Generated OpenAPI spec with {} paths across {} modules",
        spec.paths.len(),
        spec.tags.as_ref().map(|t| t.len()).unwrap_or(0)
    );

    Ok(spec)
}

/// Build OpenAPI specification from Matomo methods
fn build_openapi_spec(methods: &[MatomoMethod], base_url: &str, version: &str) -> OpenApiSpec {
    let mut paths: IndexMap<String, PathItem> = IndexMap::new();
    let mut tags_set: HashMap<String, Tag> = HashMap::new();

    for method in methods {
        let operation = create_operation(method);

        // Add tag for this module
        if !tags_set.contains_key(&method.module) {
            tags_set.insert(
                method.module.clone(),
                Tag {
                    name: method.module.clone(),
                    description: Some(format!("{} module API methods", method.module)),
                },
            );
        }

        // Add operation to path
        let method_path = format!(
            "/index.php?module=API&method={}.{}&format=json",
            method.module, method.action
        );

        paths.insert(
            method_path,
            PathItem {
                get: Some(operation),
                post: None,
            },
        );
    }

    // Collect tags
    let tags: Vec<Tag> = tags_set.into_values().collect();

    // Build security schemes
    let mut security_schemes = HashMap::new();
    security_schemes.insert(
        "token_auth".to_string(),
        SecurityScheme {
            scheme_type: "apiKey".to_string(),
            description: Some("Matomo authentication token".to_string()),
            name: Some("token_auth".to_string()),
            location: Some("query".to_string()),
            scheme: None,
        },
    );

    OpenApiSpec {
        openapi: "3.0.3".to_string(),
        info: Info {
            title: "Matomo Analytics API".to_string(),
            description: Some(
                "Auto-generated OpenAPI specification for Matomo Analytics API. \
                 Generated dynamically by introspecting the Matomo API endpoints."
                    .to_string(),
            ),
            version: version.to_string(),
        },
        servers: vec![Server {
            url: base_url.to_string(),
            description: Some("Matomo instance".to_string()),
        }],
        paths,
        components: Some(Components {
            schemas: None,
            security_schemes: Some(security_schemes),
        }),
        tags: Some(tags),
    }
}

/// Create an OpenAPI operation from a Matomo method
fn create_operation(method: &MatomoMethod) -> Operation {
    let operation_id = format!("{}_{}", method.module, method.action);
    let summary = Some(format!("{}.{}", method.module, method.action));

    // Convert parameters
    let parameters: Vec<Parameter> = method
        .parameters
        .iter()
        .map(convert_to_openapi_parameter)
        .collect();

    // Build response schema
    let response_schema = method
        .response_schema
        .clone()
        .unwrap_or_else(|| JsonSchema {
            schema_type: "object".to_string(),
            description: Some("API response".to_string()),
            ..Default::default()
        });

    let mut content = HashMap::new();
    content.insert(
        "application/json".to_string(),
        crate::openapi::MediaType {
            schema: serde_json::to_value(&response_schema).unwrap_or_default(),
            example: method.example_response.clone(),
        },
    );

    let mut responses = IndexMap::new();
    responses.insert(
        "200".to_string(),
        Response {
            description: "Successful response".to_string(),
            content: Some(content),
        },
    );
    responses.insert(
        "400".to_string(),
        Response {
            description: "Bad request - invalid parameters".to_string(),
            content: None,
        },
    );
    responses.insert(
        "401".to_string(),
        Response {
            description: "Unauthorized - authentication required".to_string(),
            content: None,
        },
    );

    Operation {
        operation_id,
        summary,
        description: method.description.clone(),
        tags: Some(vec![method.module.clone()]),
        parameters: if parameters.is_empty() {
            None
        } else {
            Some(parameters)
        },
        responses,
    }
}

/// Convert a Matomo parameter to an OpenAPI parameter
fn convert_to_openapi_parameter(param: &MatomoParameter) -> Parameter {
    let (schema_type, format) = param.param_type.to_openapi_type();

    let default = param
        .default_value
        .as_ref()
        .map(|d| match param.param_type {
            crate::types::ParameterType::Integer => d
                .parse::<i64>()
                .map(|n| serde_json::Value::Number(n.into()))
                .unwrap_or(serde_json::Value::String(d.clone())),
            crate::types::ParameterType::Float => d
                .parse::<f64>()
                .map(|n| {
                    serde_json::Number::from_f64(n)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::String(d.clone()))
                })
                .unwrap_or(serde_json::Value::String(d.clone())),
            crate::types::ParameterType::Boolean => {
                serde_json::Value::Bool(d == "true" || d == "1")
            }
            _ => serde_json::Value::String(d.clone()),
        });

    // Add enum values for known parameter types
    let enum_values = get_enum_values(&param.name);

    Parameter {
        name: param.name.clone(),
        location: "query".to_string(),
        description: param.description.clone(),
        required: param.required,
        schema: ParameterSchema {
            schema_type: schema_type.to_string(),
            format: format.map(|s| s.to_string()),
            default,
            enum_values,
        },
        example: None,
    }
}

/// Get enum values for known Matomo parameters
fn get_enum_values(param_name: &str) -> Option<Vec<String>> {
    match param_name {
        "period" => Some(vec![
            "day".to_string(),
            "week".to_string(),
            "month".to_string(),
            "year".to_string(),
            "range".to_string(),
        ]),
        "format" => Some(vec![
            "JSON".to_string(),
            "XML".to_string(),
            "CSV".to_string(),
            "TSV".to_string(),
            "HTML".to_string(),
            "PHP".to_string(),
            "RSS".to_string(),
        ]),
        _ => None,
    }
}
