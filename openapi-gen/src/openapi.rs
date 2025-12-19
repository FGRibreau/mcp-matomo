use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{JsonSchema, MatomoMethod, MatomoParameter, ParameterType};

/// OpenAPI 3.0 specification root
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
    pub description: Option<String>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<Contact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
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
    pub schema: JsonSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Components {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schemas: Option<HashMap<String, JsonSchema>>,
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

/// Generate OpenAPI specification from Matomo methods
pub fn generate_openapi(methods: &[MatomoMethod], base_url: &str, version: &str) -> OpenApiSpec {
    let mut paths: IndexMap<String, PathItem> = IndexMap::new();
    let mut tags_set: HashMap<String, Tag> = HashMap::new();

    // Generate paths for each method
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
        // Matomo API uses GET for most methods, but we'll add it as a unique path using the method name
        let method_path = format!("/index.php?module=API&method={}.{}&format=json", method.module, method.action);

        paths.insert(
            method_path,
            PathItem {
                get: Some(operation),
                post: None,
                put: None,
                delete: None,
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
    security_schemes.insert(
        "cookieAuth".to_string(),
        SecurityScheme {
            scheme_type: "apiKey".to_string(),
            description: Some("Session cookie authentication".to_string()),
            name: Some("MATOMO_SESSID".to_string()),
            location: Some("cookie".to_string()),
            scheme: None,
        },
    );

    OpenApiSpec {
        openapi: "3.0.3".to_string(),
        info: Info {
            title: "Matomo Analytics API".to_string(),
            description: Some(
                "Auto-generated OpenAPI specification for Matomo Analytics API. \
                 This specification was generated by introspecting the Matomo API endpoints."
                    .to_string(),
            ),
            version: version.to_string(),
            contact: Some(Contact {
                name: Some("Matomo".to_string()),
                url: Some("https://matomo.org".to_string()),
                email: None,
            }),
            license: Some(License {
                name: "GPL-3.0".to_string(),
                url: Some("https://www.gnu.org/licenses/gpl-3.0.html".to_string()),
            }),
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
        .map(|p| convert_to_openapi_parameter(p))
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
        MediaType {
            schema: response_schema,
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

    // Security requirement
    let mut token_auth = HashMap::new();
    token_auth.insert("token_auth".to_string(), vec![]);

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
        security: Some(vec![token_auth]),
    }
}

/// Convert a Matomo parameter to an OpenAPI parameter
fn convert_to_openapi_parameter(param: &MatomoParameter) -> Parameter {
    let (schema_type, format) = param.param_type.to_openapi_type();

    let default = param.default_value.as_ref().map(|d| {
        match param.param_type {
            ParameterType::Integer => d.parse::<i64>().map(|n| serde_json::Value::Number(n.into())).unwrap_or(serde_json::Value::String(d.clone())),
            ParameterType::Float => d.parse::<f64>().map(|n| serde_json::Number::from_f64(n).map(serde_json::Value::Number).unwrap_or(serde_json::Value::String(d.clone()))).unwrap_or(serde_json::Value::String(d.clone())),
            ParameterType::Boolean => serde_json::Value::Bool(d == "true" || d == "1"),
            _ => serde_json::Value::String(d.clone()),
        }
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
