use std::collections::HashMap;

use crate::types::JsonSchema;

/// Infer a JSON schema from a JSON value
pub fn infer_schema(value: &serde_json::Value) -> JsonSchema {
    match value {
        serde_json::Value::Null => JsonSchema {
            schema_type: "null".to_string(),
            nullable: Some(true),
            ..Default::default()
        },

        serde_json::Value::Bool(_) => JsonSchema {
            schema_type: "boolean".to_string(),
            ..Default::default()
        },

        serde_json::Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                JsonSchema {
                    schema_type: "integer".to_string(),
                    format: Some("int64".to_string()),
                    ..Default::default()
                }
            } else {
                JsonSchema {
                    schema_type: "number".to_string(),
                    format: Some("double".to_string()),
                    ..Default::default()
                }
            }
        }

        serde_json::Value::String(s) => infer_string_schema(s),

        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                JsonSchema {
                    schema_type: "array".to_string(),
                    items: Some(Box::new(JsonSchema {
                        schema_type: "object".to_string(),
                        ..Default::default()
                    })),
                    ..Default::default()
                }
            } else {
                // Infer schema from array elements, merge if different types
                let item_schemas: Vec<JsonSchema> = arr.iter().map(infer_schema).collect();
                let merged_item_schema = merge_schemas(&item_schemas);

                JsonSchema {
                    schema_type: "array".to_string(),
                    items: Some(Box::new(merged_item_schema)),
                    ..Default::default()
                }
            }
        }

        serde_json::Value::Object(obj) => {
            let mut properties = HashMap::new();

            for (key, val) in obj {
                let prop_schema = infer_schema(val);
                properties.insert(key.clone(), prop_schema);

                // Consider all properties as optional since we're inferring from examples
                // In real usage, some properties might always be present
            }

            JsonSchema {
                schema_type: "object".to_string(),
                properties: if properties.is_empty() {
                    None
                } else {
                    Some(properties)
                },
                required: None,
                ..Default::default()
            }
        }
    }
}

/// Infer schema for a string value, detecting date/time formats
fn infer_string_schema(s: &str) -> JsonSchema {
    // Check for common date/time patterns
    if is_date(s) {
        return JsonSchema {
            schema_type: "string".to_string(),
            format: Some("date".to_string()),
            ..Default::default()
        };
    }

    if is_datetime(s) {
        return JsonSchema {
            schema_type: "string".to_string(),
            format: Some("date-time".to_string()),
            ..Default::default()
        };
    }

    if is_url(s) {
        return JsonSchema {
            schema_type: "string".to_string(),
            format: Some("uri".to_string()),
            ..Default::default()
        };
    }

    if is_email(s) {
        return JsonSchema {
            schema_type: "string".to_string(),
            format: Some("email".to_string()),
            ..Default::default()
        };
    }

    // Check if it looks like a number represented as string
    if s.parse::<i64>().is_ok() {
        return JsonSchema {
            schema_type: "string".to_string(),
            description: Some("Numeric string".to_string()),
            ..Default::default()
        };
    }

    JsonSchema {
        schema_type: "string".to_string(),
        ..Default::default()
    }
}

/// Check if string looks like a date (YYYY-MM-DD)
fn is_date(s: &str) -> bool {
    let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").ok();
    re.map(|r| r.is_match(s)).unwrap_or(false)
}

/// Check if string looks like a datetime
fn is_datetime(s: &str) -> bool {
    let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}").ok();
    re.map(|r| r.is_match(s)).unwrap_or(false)
}

/// Check if string looks like a URL
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Check if string looks like an email
fn is_email(s: &str) -> bool {
    s.contains('@') && s.contains('.')
}

/// Merge multiple schemas into one (for array elements with varying types)
fn merge_schemas(schemas: &[JsonSchema]) -> JsonSchema {
    if schemas.is_empty() {
        return JsonSchema::default();
    }

    if schemas.len() == 1 {
        return schemas[0].clone();
    }

    // Check if all schemas have the same type
    let first_type = &schemas[0].schema_type;
    let all_same_type = schemas.iter().all(|s| &s.schema_type == first_type);

    if all_same_type {
        match first_type.as_str() {
            "object" => {
                // Merge object properties
                let mut merged_props: HashMap<String, JsonSchema> = HashMap::new();

                for schema in schemas {
                    if let Some(props) = &schema.properties {
                        for (key, prop_schema) in props {
                            merged_props.insert(key.clone(), prop_schema.clone());
                        }
                    }
                }

                JsonSchema {
                    schema_type: "object".to_string(),
                    properties: if merged_props.is_empty() {
                        None
                    } else {
                        Some(merged_props)
                    },
                    ..Default::default()
                }
            }
            _ => schemas[0].clone(),
        }
    } else {
        // Use anyOf for mixed types
        JsonSchema {
            schema_type: "object".to_string(), // OpenAPI 3.0 quirk
            any_of: Some(schemas.to_vec()),
            ..Default::default()
        }
    }
}

/// Analyze multiple example responses to build a more complete schema
#[allow(dead_code)]
pub fn analyze_responses(responses: &[serde_json::Value]) -> JsonSchema {
    let schemas: Vec<JsonSchema> = responses
        .iter()
        .filter(|v| !v.is_null())
        .map(infer_schema)
        .collect();

    if schemas.is_empty() {
        return JsonSchema {
            schema_type: "object".to_string(),
            description: Some("No example responses available".to_string()),
            ..Default::default()
        };
    }

    merge_schemas(&schemas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_infer_schema_primitive() {
        assert_eq!(infer_schema(&json!(true)).schema_type, "boolean");
        assert_eq!(infer_schema(&json!(42)).schema_type, "integer");
        assert_eq!(infer_schema(&json!(3.14)).schema_type, "number");
        assert_eq!(infer_schema(&json!("hello")).schema_type, "string");
    }

    #[test]
    fn test_infer_schema_array() {
        let schema = infer_schema(&json!([1, 2, 3]));
        assert_eq!(schema.schema_type, "array");
        assert!(schema.items.is_some());
    }

    #[test]
    fn test_infer_schema_object() {
        let schema = infer_schema(&json!({"name": "test", "value": 123}));
        assert_eq!(schema.schema_type, "object");
        assert!(schema.properties.is_some());
        let props = schema.properties.unwrap();
        assert!(props.contains_key("name"));
        assert!(props.contains_key("value"));
    }

    #[test]
    fn test_infer_date_format() {
        let schema = infer_schema(&json!("2024-01-15"));
        assert_eq!(schema.format, Some("date".to_string()));
    }
}
