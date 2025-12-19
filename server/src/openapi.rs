use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenAPI 3.0 specification (subset for our needs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: Info,
    pub servers: Vec<Server>,
    pub paths: IndexMap<String, PathItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Components>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub operation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<Parameter>>,
    pub responses: IndexMap<String, Response>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub required: bool,
    pub schema: ParameterSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HashMap<String, MediaType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    pub schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schemas: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<HashMap<String, SecurityScheme>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScheme {
    #[serde(rename = "type")]
    pub scheme_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "in")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parsed tool from OpenAPI spec
#[derive(Debug, Clone)]
pub struct MatomoTool {
    pub name: String,
    pub module: String,
    pub action: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    pub param_type: String,
    pub default: Option<serde_json::Value>,
    pub enum_values: Option<Vec<String>>,
}

impl OpenApiSpec {
    /// Load OpenAPI spec from a JSON file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let spec: OpenApiSpec = serde_json::from_str(&content)?;
        Ok(spec)
    }

    /// Extract all tools from the OpenAPI spec
    pub fn extract_tools(&self) -> Vec<MatomoTool> {
        let mut tools = Vec::new();

        for (_path, path_item) in &self.paths {
            // Get the operation (prefer GET, fallback to POST)
            let operation = path_item.get.as_ref().or(path_item.post.as_ref());

            if let Some(op) = operation {
                // Parse operation_id to get module and action
                // Format: "Module_action" -> module="Module", action="action"
                let parts: Vec<&str> = op.operation_id.splitn(2, '_').collect();
                let (module, action) = if parts.len() == 2 {
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    (op.operation_id.clone(), op.operation_id.clone())
                };

                // Build description
                let description = op
                    .description
                    .clone()
                    .or(op.summary.clone())
                    .unwrap_or_else(|| format!("Call {}.{}", module, action));

                // Extract parameters
                let parameters: Vec<ToolParameter> = op
                    .parameters
                    .as_ref()
                    .map(|params| {
                        params
                            .iter()
                            .map(|p| ToolParameter {
                                name: p.name.clone(),
                                description: p.description.clone(),
                                required: p.required,
                                param_type: p.schema.schema_type.clone(),
                                default: p.schema.default.clone(),
                                enum_values: p.schema.enum_values.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                tools.push(MatomoTool {
                    name: op.operation_id.clone(),
                    module,
                    action,
                    description,
                    parameters,
                });
            }
        }

        tools
    }

    /// Get the base URL from servers
    pub fn get_base_url(&self) -> Option<String> {
        self.servers.first().map(|s| s.url.clone())
    }
}
