use crate::matomo_client::MatomoClient;
use crate::openapi::{MatomoTool, OpenApiSpec};
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ErrorData;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// MCP Service for Matomo Analytics
#[derive(Clone)]
pub struct MatomoService {
    /// Matomo HTTP client
    client: Arc<MatomoClient>,
    /// Available tools parsed from OpenAPI spec
    tools: Arc<Vec<MatomoTool>>,
    /// Server info
    matomo_version: String,
    matomo_url: String,
}

impl MatomoService {
    /// Create a new MatomoService from OpenAPI spec
    pub fn new(spec: OpenApiSpec, token: Option<String>) -> anyhow::Result<Self> {
        let base_url = spec
            .get_base_url()
            .ok_or_else(|| anyhow::anyhow!("No server URL in OpenAPI spec"))?;

        let client = MatomoClient::new(&base_url, token)?;
        let tools = spec.extract_tools();

        info!("Loaded {} tools from OpenAPI spec", tools.len());

        Ok(Self {
            client: Arc::new(client),
            tools: Arc::new(tools),
            matomo_version: spec.info.version.clone(),
            matomo_url: base_url,
        })
    }

    /// Find a tool by name
    fn find_tool(&self, name: &str) -> Option<&MatomoTool> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Convert MatomoTool to MCP Tool definition
    fn tool_to_mcp(&self, tool: &MatomoTool) -> Tool {
        // Build JSON schema for parameters
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &tool.parameters {
            let mut prop = serde_json::Map::new();

            // Map OpenAPI type to JSON Schema type
            let json_type = match param.param_type.as_str() {
                "integer" => "integer",
                "number" => "number",
                "boolean" => "boolean",
                "array" => "array",
                "object" => "object",
                _ => "string",
            };
            prop.insert(
                "type".to_string(),
                serde_json::Value::String(json_type.to_string()),
            );

            if let Some(ref desc) = param.description {
                prop.insert(
                    "description".to_string(),
                    serde_json::Value::String(desc.clone()),
                );
            }

            if let Some(ref default) = param.default {
                prop.insert("default".to_string(), default.clone());
            }

            if let Some(ref enum_vals) = param.enum_values {
                let enum_arr: Vec<serde_json::Value> = enum_vals
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect();
                prop.insert("enum".to_string(), serde_json::Value::Array(enum_arr));
            }

            properties.insert(param.name.clone(), serde_json::Value::Object(prop));

            if param.required {
                required.push(param.name.clone());
            }
        }

        let mut schema = serde_json::Map::new();
        schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
        schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(properties),
        );

        if !required.is_empty() {
            let required_arr: Vec<serde_json::Value> = required
                .iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect();
            schema.insert(
                "required".to_string(),
                serde_json::Value::Array(required_arr),
            );
        }

        Tool {
            name: Cow::Owned(tool.name.clone()),
            description: Some(Cow::Owned(tool.description.clone())),
            input_schema: Arc::new(schema),
            annotations: None,
            icons: None,
            meta: None,
            output_schema: None,
            title: None,
        }
    }
}

impl ServerHandler for MatomoService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "mcp-matomo".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                title: None,
                website_url: None,
            },
            instructions: Some(format!(
                "Matomo Analytics API server.\n\
                 Connected to: {}\n\
                 Matomo version: {}\n\
                 Available tools: {}\n\n\
                 Use these tools to query analytics data from your Matomo instance.",
                self.matomo_url,
                self.matomo_version,
                self.tools.len()
            )),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        debug!("Listing {} tools", self.tools.len());
        let tools: Vec<Tool> = self.tools.iter().map(|t| self.tool_to_mcp(t)).collect();
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let tool_name = request.name.as_ref();
        debug!("Calling tool: {}", tool_name);

        // Find the tool
        let tool = self.find_tool(tool_name).ok_or_else(|| {
            ErrorData::invalid_params(format!("Unknown tool: {}", tool_name), None)
        })?;

        // Extract parameters from arguments
        let params: HashMap<String, serde_json::Value> = match request.arguments {
            Some(map) => map.into_iter().collect(),
            None => HashMap::new(),
        };

        // Call Matomo API
        match self
            .client
            .call_method(&tool.module, &tool.action, params)
            .await
        {
            Ok(result) => {
                // Format the response nicely
                let text =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());

                Ok(CallToolResult {
                    content: vec![Content::text(text)],
                    is_error: Some(false),
                    meta: None,
                    structured_content: None,
                })
            }
            Err(e) => Ok(CallToolResult {
                content: vec![Content::text(format!("Error: {}", e))],
                is_error: Some(true),
                meta: None,
                structured_content: None,
            }),
        }
    }
}
