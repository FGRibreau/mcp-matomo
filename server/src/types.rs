use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a Matomo API method with its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatomoMethod {
    /// Full method name (e.g., "API.getMatomoVersion")
    pub name: String,
    /// Module name (e.g., "API")
    pub module: String,
    /// Action name (e.g., "getMatomoVersion")
    pub action: String,
    /// Parameters for this method
    pub parameters: Vec<MatomoParameter>,
    /// Example response (JSON value)
    pub example_response: Option<serde_json::Value>,
    /// Inferred response schema
    pub response_schema: Option<JsonSchema>,
    /// Method description/documentation
    pub description: Option<String>,
    /// Method category
    pub category: Option<String>,
}

/// Represents a parameter for a Matomo API method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatomoParameter {
    pub name: String,
    pub required: bool,
    pub param_type: ParameterType,
    pub default_value: Option<String>,
    pub description: Option<String>,
}

/// Possible parameter types in Matomo API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParameterType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    Array,
    Object,
    Unknown,
}

impl ParameterType {
    pub fn to_openapi_type(&self) -> (&'static str, Option<&'static str>) {
        match self {
            ParameterType::String => ("string", None),
            ParameterType::Integer => ("integer", Some("int64")),
            ParameterType::Float => ("number", Some("double")),
            ParameterType::Boolean => ("boolean", None),
            ParameterType::Date => ("string", Some("date")),
            ParameterType::Array => ("array", None),
            ParameterType::Object => ("object", None),
            ParameterType::Unknown => ("string", None),
        }
    }
}

/// JSON Schema representation for OpenAPI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<Box<JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nullable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_of: Option<Vec<JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<JsonSchema>>,
}

impl Default for JsonSchema {
    fn default() -> Self {
        JsonSchema {
            schema_type: "object".to_string(),
            format: None,
            items: None,
            properties: None,
            additional_properties: None,
            required: None,
            description: None,
            enum_values: None,
            nullable: None,
            one_of: None,
            any_of: None,
        }
    }
}

/// Represents method metadata from the API documentation page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodMetadata {
    pub parameters: Vec<MethodParameter>,
    pub example_url: Option<String>,
}

/// Parameter from the documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodParameter {
    pub name: String,
    pub required: bool,
    pub default: Option<String>,
}
