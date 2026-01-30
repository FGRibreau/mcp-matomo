//! Shared HTTP client configuration for Matomo API requests
//!
//! This module provides a centralized HTTP client builder with:
//! - Custom User-Agent header (mcp-matomo/<version>)
//! - Support for extra headers via MCP_MATOMO_EXTRA_HEADERS env var

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use std::env;
use std::time::Duration;
use tracing::debug;

/// Environment variable name for extra headers
pub const EXTRA_HEADERS_ENV: &str = "MCP_MATOMO_EXTRA_HEADERS";

/// Default User-Agent for mcp-matomo requests
pub fn user_agent() -> String {
    format!("mcp-matomo/{}", env!("CARGO_PKG_VERSION"))
}

/// Parse extra headers from environment variable
///
/// Format: "Header1:Value1,Header2:Value2"
/// Example: "X-Custom-Auth:secret123,X-Tenant-Id:abc"
pub fn parse_extra_headers(env_value: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    if env_value.trim().is_empty() {
        return Ok(headers);
    }

    for header_pair in env_value.split(',') {
        let header_pair = header_pair.trim();
        if header_pair.is_empty() {
            continue;
        }

        let (name, value) = header_pair.split_once(':').with_context(|| {
            format!(
                "Invalid header format '{}', expected 'Name:Value'",
                header_pair
            )
        })?;

        let name = name.trim();
        let value = value.trim();

        let header_name = HeaderName::try_from(name)
            .with_context(|| format!("Invalid header name: '{}'", name))?;
        let header_value = HeaderValue::try_from(value)
            .with_context(|| format!("Invalid header value for '{}': '{}'", name, value))?;

        headers.insert(header_name, header_value);
    }

    Ok(headers)
}

/// Parse extra headers from CLI arguments (Vec of "Key:Value" strings)
///
/// Each entry should be in "Header:Value" format.
/// Example: vec!["X-Custom-Auth:secret123", "X-Tenant-Id:abc"]
pub fn parse_cli_headers(header_args: &[String]) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    for header_pair in header_args {
        let header_pair = header_pair.trim();
        if header_pair.is_empty() {
            continue;
        }

        let (name, value) = header_pair.split_once(':').with_context(|| {
            format!(
                "Invalid header format '{}', expected 'Name:Value'",
                header_pair
            )
        })?;

        let name = name.trim();
        let value = value.trim();

        let header_name = HeaderName::try_from(name)
            .with_context(|| format!("Invalid header name: '{}'", name))?;
        let header_value = HeaderValue::try_from(value)
            .with_context(|| format!("Invalid header value for '{}': '{}'", name, value))?;

        headers.insert(header_name, header_value);
    }

    Ok(headers)
}

/// Get extra headers from environment variable
pub fn get_extra_headers_from_env() -> Result<HeaderMap> {
    match env::var(EXTRA_HEADERS_ENV) {
        Ok(value) => {
            debug!("Parsing extra headers from {}", EXTRA_HEADERS_ENV);
            parse_extra_headers(&value)
        }
        Err(env::VarError::NotPresent) => Ok(HeaderMap::new()),
        Err(e) => Err(anyhow::anyhow!(
            "Failed to read {}: {}",
            EXTRA_HEADERS_ENV,
            e
        )),
    }
}

/// Build HTTP client with mcp-matomo configuration
///
/// Configuration includes:
/// - Custom User-Agent: mcp-matomo/<version>
/// - Extra headers from MCP_MATOMO_EXTRA_HEADERS env var
/// - Extra headers from CLI arguments (merged, CLI takes precedence)
/// - 60 second timeout
/// - Optional: accept invalid certificates (for self-signed certs)
pub fn build_client(accept_invalid_certs: bool, cli_headers: &HeaderMap) -> Result<Client> {
    let mut default_headers = get_extra_headers_from_env()?;

    // Merge CLI headers (CLI takes precedence over env)
    for (name, value) in cli_headers.iter() {
        default_headers.insert(name, value.clone());
    }

    // Add User-Agent to default headers
    default_headers.insert(
        reqwest::header::USER_AGENT,
        HeaderValue::try_from(user_agent()).expect("User-Agent is always valid"),
    );

    let mut builder = Client::builder()
        .timeout(Duration::from_secs(60))
        .default_headers(default_headers);

    if accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder.build().context("Failed to build HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_agent_format() {
        let ua = user_agent();
        assert!(ua.starts_with("mcp-matomo/"));
        assert!(ua.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_parse_extra_headers_empty() {
        let headers = parse_extra_headers("").unwrap();
        assert!(headers.is_empty());
    }

    #[test]
    fn test_parse_extra_headers_single() {
        let headers = parse_extra_headers("X-Custom-Header:value123").unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("X-Custom-Header").unwrap(), "value123");
    }

    #[test]
    fn test_parse_extra_headers_multiple() {
        let headers =
            parse_extra_headers("X-Auth:token123,X-Tenant:abc,Accept-Language:fr").unwrap();
        assert_eq!(headers.len(), 3);
        assert_eq!(headers.get("X-Auth").unwrap(), "token123");
        assert_eq!(headers.get("X-Tenant").unwrap(), "abc");
        assert_eq!(headers.get("Accept-Language").unwrap(), "fr");
    }

    #[test]
    fn test_parse_extra_headers_with_spaces() {
        let headers =
            parse_extra_headers("  X-Header : value with spaces  , X-Other: test  ").unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers.get("X-Header").unwrap(), "value with spaces");
        assert_eq!(headers.get("X-Other").unwrap(), "test");
    }

    #[test]
    fn test_parse_extra_headers_colon_in_value() {
        let headers = parse_extra_headers("Authorization:Bearer:abc:123").unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer:abc:123");
    }

    #[test]
    fn test_parse_extra_headers_invalid_format() {
        let result = parse_extra_headers("InvalidHeaderWithoutColon");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid header format"));
    }

    #[test]
    fn test_parse_extra_headers_invalid_header_name() {
        let result = parse_extra_headers("Invalid Header Name:value");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid header name"));
    }

    #[test]
    fn test_build_client_without_invalid_certs() {
        let client = build_client(false, &HeaderMap::new());
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_client_with_invalid_certs() {
        let client = build_client(true, &HeaderMap::new());
        assert!(client.is_ok());
    }

    #[test]
    fn test_parse_cli_headers_empty() {
        let headers = parse_cli_headers(&[]).unwrap();
        assert!(headers.is_empty());
    }

    #[test]
    fn test_parse_cli_headers_single() {
        let headers = parse_cli_headers(&["X-Custom:value123".to_string()]).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("X-Custom").unwrap(), "value123");
    }

    #[test]
    fn test_parse_cli_headers_multiple() {
        let headers = parse_cli_headers(&[
            "X-Auth:token123".to_string(),
            "X-Tenant:abc".to_string(),
            "Accept-Language:fr".to_string(),
        ])
        .unwrap();
        assert_eq!(headers.len(), 3);
        assert_eq!(headers.get("X-Auth").unwrap(), "token123");
        assert_eq!(headers.get("X-Tenant").unwrap(), "abc");
        assert_eq!(headers.get("Accept-Language").unwrap(), "fr");
    }

    #[test]
    fn test_parse_cli_headers_colon_in_value() {
        let headers = parse_cli_headers(&["Authorization:Bearer:abc:123".to_string()]).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer:abc:123");
    }

    #[test]
    fn test_parse_cli_headers_invalid_format() {
        let result = parse_cli_headers(&["InvalidHeaderWithoutColon".to_string()]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid header format"));
    }

    #[test]
    fn test_build_client_with_cli_headers() {
        let mut cli_headers = HeaderMap::new();
        cli_headers.insert("X-Custom", HeaderValue::from_static("test"));
        let client = build_client(false, &cli_headers);
        assert!(client.is_ok());
    }
}
