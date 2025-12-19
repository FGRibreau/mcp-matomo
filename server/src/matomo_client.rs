use anyhow::{Context, Result};
use reqwest::Client;
use std::collections::HashMap;
use tracing::debug;
use url::Url;

/// HTTP client for making Matomo API calls
#[derive(Debug, Clone)]
pub struct MatomoClient {
    client: Client,
    base_url: Url,
    token_auth: Option<String>,
}

impl MatomoClient {
    /// Create a new Matomo client
    pub fn new(base_url: &str, token: Option<String>) -> Result<Self> {
        let base_url = Url::parse(base_url).context("Invalid base URL")?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            client,
            base_url,
            token_auth: token,
        })
    }

    /// Call a Matomo API method
    pub async fn call_method(
        &self,
        module: &str,
        action: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let mut url = self.base_url.clone();
        url.set_path("index.php");

        let method_str = format!("{}.{}", module, action);
        debug!("Calling Matomo API: {}", method_str);

        // Build form parameters
        let mut form_params: Vec<(String, String)> = vec![
            ("module".to_string(), "API".to_string()),
            ("method".to_string(), method_str),
            ("format".to_string(), "JSON".to_string()),
        ];

        // Add token if available
        if let Some(ref token) = self.token_auth {
            form_params.push(("token_auth".to_string(), token.clone()));
        }

        // Add user-provided parameters
        for (key, value) in params {
            let str_value = match value {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => if b { "1".to_string() } else { "0".to_string() },
                serde_json::Value::Null => continue,
                other => other.to_string(),
            };
            form_params.push((key, str_value));
        }

        // Make POST request (required for token_auth)
        let response = self.client
            .post(url.as_str())
            .form(&form_params)
            .send()
            .await
            .context("Failed to send request to Matomo")?;

        let status = response.status();
        let text = response.text().await.context("Failed to read response")?;

        if !status.is_success() {
            anyhow::bail!("Matomo API error ({}): {}", status, text);
        }

        // Try to parse as JSON
        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| serde_json::Value::String(text));

        // Check for Matomo error response
        if let Some(obj) = json.as_object() {
            if obj.get("result").and_then(|v| v.as_str()) == Some("error") {
                let message = obj.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                anyhow::bail!("Matomo API error: {}", message);
            }
        }

        Ok(json)
    }
}
