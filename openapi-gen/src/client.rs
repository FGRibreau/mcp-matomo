use anyhow::{Context, Result};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use std::time::Duration;
use tracing::{debug, info, warn};
use url::Url;

/// HTTP client configured with cookies or token for Matomo API access
pub struct MatomoClient {
    client: Client,
    base_url: Url,
    token_auth: Option<String>,
}

impl MatomoClient {
    /// Create a new Matomo client with the given base URL, cookies, and/or token
    pub fn new(base_url: &str, cookies: &str, token: Option<&str>) -> Result<Self> {
        let base_url = Url::parse(base_url).context("Invalid base URL")?;

        let mut headers = HeaderMap::new();
        if !cookies.is_empty() {
            headers.insert(
                COOKIE,
                HeaderValue::from_str(cookies).context("Invalid cookie header")?,
            );
        }

        let client = ClientBuilder::new()
            .default_headers(headers)
            .timeout(Duration::from_secs(60))
            .danger_accept_invalid_certs(true) // Some Matomo instances have self-signed certs
            .build()
            .context("Failed to build HTTP client")?;

        Ok(MatomoClient {
            client,
            base_url,
            token_auth: token.map(|t| t.to_string()),
        })
    }

    /// Fetch the list of all API methods using API.getReportMetadata
    pub fn fetch_method_list(&self, id_site: &str) -> Result<serde_json::Value> {
        let extra_params = [("idSite", id_site)];
        let response = self.api_request("API", "getReportMetadata", &extra_params)
            .context("Failed to fetch method list")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();

            if status == reqwest::StatusCode::UNAUTHORIZED {
                let error_msg = format!(
                    r#"Authentication failed (HTTP 401)

The Matomo API returned: {}

How to fix this:

1. Use an API token (recommended):
   - Go to Matomo > Settings > Personal > Security
   - Create or copy your API token
   - Use: --token "your_token_here" (or -t "your_token_here")

2. Or use a session cookie:
   - Log in to your Matomo instance in a browser
   - Open Developer Tools (F12) > Application > Cookies
   - Copy the MATOMO_SESSID cookie value
   - Use: --cookies "MATOMO_SESSID=<your_cookie_value>"

3. Verify you have 'view' access to site ID specified with --site-id (-s):
   - Check your user permissions in Matomo > Settings > Users
   - Try a different site ID if you have access to multiple sites

4. If using a self-hosted instance, ensure the API is enabled:
   - Check Matomo > Settings > General > API"#,
                    body
                );
                anyhow::bail!(error_msg);
            }

            anyhow::bail!("HTTP error {}: {}", status, body);
        }

        let text = response.text().context("Failed to read response body")?;
        debug!("Method list response: {}", &text[..text.len().min(500)]);

        serde_json::from_str(&text).context("Failed to parse method list JSON")
    }

    /// Fetch the Matomo version
    pub fn fetch_version(&self) -> Result<String> {
        let response = self.api_request("API", "getMatomoVersion", &[])
            .context("Failed to fetch Matomo version")?;

        let status = response.status();
        if !status.is_success() {
            warn!("Could not fetch Matomo version: {}", status);
            return Ok("unknown".to_string());
        }

        let json: serde_json::Value = response.json().context("Failed to parse version JSON")?;
        Ok(json.get("value").and_then(|v| v.as_str()).unwrap_or("unknown").to_string())
    }

    /// Fetch the API documentation page for a specific method
    #[allow(dead_code)]
    pub fn fetch_method_doc(&self, module: &str, action: &str) -> Result<String> {
        debug!("Fetching method doc for {}.{}", module, action);

        let response = self.api_request("API", "listAllAPI", &[])
            .context("Failed to fetch API documentation")?;

        response.text().context("Failed to read documentation response")
    }

    /// Fetch an example response for a given method
    pub fn fetch_example(&self, module: &str, action: &str, extra_params: &[(&str, &str)]) -> Result<serde_json::Value> {
        let response = self.api_request(module, action, extra_params)
            .context("Failed to fetch example")?;

        let status = response.status();
        let text = response.text().context("Failed to read example response")?;

        if !status.is_success() {
            warn!("Example request failed for {}.{}: {} - {}", module, action, status, &text[..text.len().min(200)]);
            return Ok(serde_json::Value::Null);
        }

        // Try to parse as JSON, fall back to string value
        serde_json::from_str(&text).or_else(|_| {
            debug!("Response is not JSON, wrapping as string");
            Ok(serde_json::Value::String(text))
        })
    }

    /// Fetch the glossary/API reference page for parsing parameters
    pub fn fetch_api_reference(&self) -> Result<String> {
        let response = self.api_request("API", "listAllAPI", &[])
            .context("Failed to fetch API reference")?;

        response.text().context("Failed to read API reference")
    }

    /// Make an API request - uses POST when token is present, GET otherwise
    fn api_request(&self, module: &str, action: &str, extra_params: &[(&str, &str)]) -> Result<reqwest::blocking::Response> {
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

            info!("POST {}: {}.{}", url, module, action);

            self.client
                .post(url.as_str())
                .form(&form_params)
                .send()
                .context("Failed to send POST request")
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

            info!("GET {}", url);

            self.client
                .get(url.as_str())
                .send()
                .context("Failed to send GET request")
        }
    }

    /// Get base URL
    #[allow(dead_code)]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }
}
