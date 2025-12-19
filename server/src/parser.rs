use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::types::{MatomoParameter, MethodMetadata, MethodParameter, ParameterType};

/// Parsed report method with documentation
#[derive(Debug, Clone)]
pub struct ParsedReportMethod {
    pub module: String,
    pub action: String,
    #[allow(dead_code)]
    pub name: String,
    pub documentation: Option<String>,
    pub category: Option<String>,
}

/// Parse the method list response from Matomo API (getReportMetadata format)
pub fn parse_method_list(json: &serde_json::Value) -> Result<Vec<ParsedReportMethod>> {
    let mut methods = Vec::new();

    match json {
        serde_json::Value::Array(arr) => {
            // Format from getReportMetadata: [{"module": "...", "action": "...", "name": "...", "documentation": "..."}, ...]
            for item in arr {
                if let serde_json::Value::Object(obj) = item {
                    let module = obj
                        .get("module")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let action = obj
                        .get("action")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                    let documentation = obj
                        .get("documentation")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let category = obj
                        .get("category")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    if !module.is_empty() && !action.is_empty() {
                        methods.push(ParsedReportMethod {
                            module: module.to_string(),
                            action: action.to_string(),
                            name: name.to_string(),
                            documentation,
                            category,
                        });
                    }
                }
            }
        }
        serde_json::Value::Object(obj) => {
            // Fallback format: { "Module": ["action1", "action2", ...], ... }
            for (module, actions) in obj {
                if let serde_json::Value::Array(actions_arr) = actions {
                    for action in actions_arr {
                        if let serde_json::Value::String(action_name) = action {
                            methods.push(ParsedReportMethod {
                                module: module.clone(),
                                action: action_name.clone(),
                                name: format!("{}.{}", module, action_name),
                                documentation: None,
                                category: None,
                            });
                        }
                    }
                }
            }
        }
        _ => anyhow::bail!("Unexpected method list format"),
    }

    info!("Parsed {} methods from API", methods.len());
    Ok(methods)
}

/// Parse the API reference HTML page to extract method metadata
pub fn parse_api_reference(html: &str) -> Result<HashMap<String, MethodMetadata>> {
    let document = Html::parse_document(html);
    let mut methods = HashMap::new();

    // Selector for method headings
    let method_selector = Selector::parse("h2, h3, .apiMethod, .method-name").ok();

    // Try to parse from the raw text using regex patterns
    let method_pattern = Regex::new(r"(?m)^(\w+)\.(\w+)\s*\(?([^)]*)\)?").ok();

    if let Some(pattern) = method_pattern {
        for cap in pattern.captures_iter(html) {
            let module = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let action = cap
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let params_str = cap.get(3).map(|m| m.as_str()).unwrap_or("");

            let method_name = format!("{}.{}", module, action);
            let parameters = parse_parameters_from_signature(params_str);

            methods.insert(
                method_name,
                MethodMetadata {
                    parameters,
                    example_url: None,
                },
            );
        }
    }

    // Try parsing the formatted HTML structure
    if let Some(selector) = method_selector {
        for element in document.select(&selector) {
            let text = element.text().collect::<String>();
            if let Some((module, action)) = text.trim().split_once('.') {
                let method_name = format!("{}.{}", module.trim(), action.trim());

                methods
                    .entry(method_name)
                    .or_insert_with(|| MethodMetadata {
                        parameters: Vec::new(),
                        example_url: None,
                    });
            }
        }
    }

    debug!("Parsed {} method metadata entries from HTML", methods.len());
    Ok(methods)
}

/// Parse parameters from a method signature string like "idSite, period, date, segment = ''"
fn parse_parameters_from_signature(signature: &str) -> Vec<MethodParameter> {
    let mut params = Vec::new();

    for part in signature.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((name, default)) = part.split_once('=') {
            params.push(MethodParameter {
                name: name.trim().to_string(),
                required: false,
                default: Some(
                    default
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string(),
                ),
            });
        } else {
            params.push(MethodParameter {
                name: part.to_string(),
                required: true,
                default: None,
            });
        }
    }

    params
}

/// Infer parameter type from its name and default value
pub fn infer_parameter_type(name: &str, default: Option<&str>) -> ParameterType {
    let name_lower = name.to_lowercase();

    // Check for common patterns in Matomo API
    if name_lower.contains("id") && !name_lower.contains("ids") {
        return ParameterType::Integer;
    }
    if name_lower.ends_with("ids") {
        return ParameterType::String; // Comma-separated IDs
    }
    if name_lower.contains("date") || name_lower == "day" {
        return ParameterType::Date;
    }
    if name_lower.contains("period") {
        return ParameterType::String;
    }
    if name_lower.starts_with("is")
        || name_lower.starts_with("has")
        || name_lower.starts_with("enable")
        || name_lower.starts_with("disable")
        || name_lower.starts_with("show")
        || name_lower.starts_with("hide")
        || name_lower.starts_with("force")
        || name_lower.starts_with("keep")
    {
        return ParameterType::Boolean;
    }
    if name_lower.contains("limit")
        || name_lower.contains("offset")
        || name_lower.contains("count")
        || name_lower.contains("rows")
        || name_lower.contains("max")
        || name_lower.contains("min")
    {
        return ParameterType::Integer;
    }
    if name_lower.contains("expanded")
        || name_lower.contains("flat")
        || name_lower.contains("serialize")
    {
        return ParameterType::Boolean;
    }

    // Check default value
    if let Some(default) = default {
        if default == "true" || default == "false" || default == "0" || default == "1" {
            return ParameterType::Boolean;
        }
        if default.parse::<i64>().is_ok() {
            return ParameterType::Integer;
        }
        if default.parse::<f64>().is_ok() {
            return ParameterType::Float;
        }
    }

    ParameterType::String
}

/// Convert MethodParameter to MatomoParameter with type inference
pub fn convert_parameter(param: &MethodParameter) -> MatomoParameter {
    let param_type = infer_parameter_type(&param.name, param.default.as_deref());

    MatomoParameter {
        name: param.name.clone(),
        required: param.required,
        param_type,
        default_value: param.default.clone(),
        description: None,
    }
}

/// Build common parameters that Matomo API methods typically accept
pub fn get_common_parameters() -> Vec<MatomoParameter> {
    vec![
        MatomoParameter {
            name: "idSite".to_string(),
            required: false,
            param_type: ParameterType::Integer,
            default_value: None,
            description: Some("The site ID".to_string()),
        },
        MatomoParameter {
            name: "period".to_string(),
            required: false,
            param_type: ParameterType::String,
            default_value: None,
            description: Some("The period (day, week, month, year, range)".to_string()),
        },
        MatomoParameter {
            name: "date".to_string(),
            required: false,
            param_type: ParameterType::String,
            default_value: None,
            description: Some(
                "The date (YYYY-MM-DD or keywords like 'today', 'yesterday')".to_string(),
            ),
        },
        MatomoParameter {
            name: "segment".to_string(),
            required: false,
            param_type: ParameterType::String,
            default_value: None,
            description: Some("Segment definition".to_string()),
        },
        MatomoParameter {
            name: "format".to_string(),
            required: false,
            param_type: ParameterType::String,
            default_value: Some("JSON".to_string()),
            description: Some("Response format (JSON, XML, CSV, etc.)".to_string()),
        },
        MatomoParameter {
            name: "filter_limit".to_string(),
            required: false,
            param_type: ParameterType::Integer,
            default_value: None,
            description: Some("Limit the number of rows returned".to_string()),
        },
        MatomoParameter {
            name: "filter_offset".to_string(),
            required: false,
            param_type: ParameterType::Integer,
            default_value: Some("0".to_string()),
            description: Some("Offset for pagination".to_string()),
        },
    ]
}
