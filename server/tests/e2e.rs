//! End-to-end tests for MCP Matomo
//!
//! These tests call the real Matomo API - no mocks!
//!
//! Required environment variables:
//! - URL: Matomo instance URL (e.g., https://matomo.example.com)
//! - TOKEN: Matomo API token (token_auth)
//! - SITE_ID: Site ID to test against
//!
//! Run with:
//!   source .envrc && cargo test --test e2e
//!
//! Note: Tests are automatically skipped when environment variables are not set.

use std::collections::HashMap;

/// Test configuration from environment variables
struct TestConfig {
    url: String,
    token: String,
    site_id: String,
}

impl TestConfig {
    fn from_env() -> Option<Self> {
        let url = std::env::var("URL").ok()?;
        let token = std::env::var("TOKEN").ok()?;
        let site_id = std::env::var("SITE_ID").ok()?;
        Some(Self {
            url,
            token,
            site_id,
        })
    }
}

/// Macro to skip test if environment is not configured
macro_rules! require_env {
    () => {
        match TestConfig::from_env() {
            Some(config) => config,
            None => {
                eprintln!("Skipping test: URL, TOKEN, and SITE_ID environment variables required");
                return;
            }
        }
    };
}

/// Simple HTTP client for testing Matomo API calls directly
struct TestMatomoClient {
    client: reqwest::Client,
    base_url: String,
    token: String,
}

impl TestMatomoClient {
    fn new(base_url: &str, token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            token: token.to_string(),
        }
    }

    async fn call(
        &self,
        module: &str,
        action: &str,
        params: HashMap<String, String>,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{}/index.php", self.base_url);

        let mut form_params: Vec<(String, String)> = vec![
            ("module".to_string(), "API".to_string()),
            ("method".to_string(), format!("{}.{}", module, action)),
            ("format".to_string(), "JSON".to_string()),
            ("token_auth".to_string(), self.token.clone()),
        ];

        for (key, value) in params {
            form_params.push((key, value));
        }

        let response = self
            .client
            .post(&url)
            .form(&form_params)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("HTTP error {}: {}", status, text));
        }

        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("JSON parse error: {} - {}", e, text))?;

        // Check for Matomo error response
        if let Some(obj) = json.as_object() {
            if obj.get("result").and_then(|v| v.as_str()) == Some("error") {
                let message = obj
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                return Err(format!("Matomo API error: {}", message));
            }
        }

        Ok(json)
    }

    fn params(&self, site_id: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("idSite".to_string(), site_id.to_string());
        params.insert("period".to_string(), "day".to_string());
        params.insert("date".to_string(), "today".to_string());
        params
    }
}

// ============================================================================
// API Metadata Tests
// ============================================================================

#[tokio::test]
async fn test_api_get_matomo_version() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client.call("API", "getMatomoVersion", HashMap::new()).await;

    assert!(result.is_ok(), "API.getMatomoVersion failed: {:?}", result);
    let version = result.unwrap();
    // Matomo can return either a string or an object with a "value" field
    let version_str = if let Some(s) = version.as_str() {
        s.to_string()
    } else if let Some(obj) = version.as_object() {
        obj.get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        panic!("Unexpected version format: {:?}", version);
    };
    assert!(!version_str.is_empty(), "Version should not be empty");
    println!("Matomo version: {}", version_str);
}

#[tokio::test]
async fn test_api_get_php_version() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client.call("API", "getPhpVersion", HashMap::new()).await;

    assert!(result.is_ok(), "API.getPhpVersion failed: {:?}", result);
    let version = result.unwrap();
    // PHP version can be a string or an object with version details
    let version_str = if let Some(s) = version.as_str() {
        s.to_string()
    } else if let Some(obj) = version.as_object() {
        obj.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        panic!("Unexpected PHP version format: {:?}", version);
    };
    assert!(!version_str.is_empty(), "PHP version should not be empty");
    println!("PHP version: {}", version_str);
}

#[tokio::test]
async fn test_api_get_ip_from_header() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client.call("API", "getIpFromHeader", HashMap::new()).await;

    assert!(result.is_ok(), "API.getIpFromHeader failed: {:?}", result);
    println!("IP from header: {}", result.unwrap());
}

#[tokio::test]
async fn test_api_get_settings() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client.call("API", "getSettings", HashMap::new()).await;

    assert!(result.is_ok(), "API.getSettings failed: {:?}", result);
    let settings = result.unwrap();
    assert!(
        settings.is_object(),
        "Expected object settings, got: {:?}",
        settings
    );
}

// ============================================================================
// Visits Summary Tests
// ============================================================================

#[tokio::test]
async fn test_visits_summary_get() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("VisitsSummary", "get", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "VisitsSummary.get failed: {:?}", result);
    let summary = result.unwrap();
    println!(
        "Visits summary: {}",
        serde_json::to_string_pretty(&summary).unwrap()
    );
}

#[tokio::test]
async fn test_visits_summary_get_visits() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("VisitsSummary", "getVisits", client.params(&config.site_id))
        .await;

    assert!(
        result.is_ok(),
        "VisitsSummary.getVisits failed: {:?}",
        result
    );
    println!("Visits: {}", result.unwrap());
}

#[tokio::test]
async fn test_visits_summary_get_unique_visitors() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "VisitsSummary",
            "getUniqueVisitors",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "VisitsSummary.getUniqueVisitors failed: {:?}",
        result
    );
    println!("Unique visitors: {}", result.unwrap());
}

// ============================================================================
// Actions Tests
// ============================================================================

#[tokio::test]
async fn test_actions_get() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Actions", "get", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Actions.get failed: {:?}", result);
    println!(
        "Actions: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_actions_get_page_urls() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Actions", "getPageUrls", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Actions.getPageUrls failed: {:?}", result);
    println!(
        "Page URLs: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_actions_get_page_titles() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Actions", "getPageTitles", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Actions.getPageTitles failed: {:?}", result);
}

#[tokio::test]
async fn test_actions_get_entry_page_urls() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Actions",
            "getEntryPageUrls",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Actions.getEntryPageUrls failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_actions_get_exit_page_urls() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Actions", "getExitPageUrls", client.params(&config.site_id))
        .await;

    assert!(
        result.is_ok(),
        "Actions.getExitPageUrls failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_actions_get_downloads() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Actions", "getDownloads", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Actions.getDownloads failed: {:?}", result);
}

#[tokio::test]
async fn test_actions_get_outlinks() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Actions", "getOutlinks", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Actions.getOutlinks failed: {:?}", result);
}

// ============================================================================
// Referrers Tests
// ============================================================================

#[tokio::test]
async fn test_referrers_get() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Referrers", "get", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Referrers.get failed: {:?}", result);
    println!(
        "Referrers: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_referrers_get_all() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Referrers", "getAll", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Referrers.getAll failed: {:?}", result);
}

#[tokio::test]
async fn test_referrers_get_referrer_type() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Referrers",
            "getReferrerType",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Referrers.getReferrerType failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_referrers_get_keywords() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Referrers", "getKeywords", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Referrers.getKeywords failed: {:?}", result);
}

#[tokio::test]
async fn test_referrers_get_search_engines() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Referrers",
            "getSearchEngines",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Referrers.getSearchEngines failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_referrers_get_websites() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Referrers", "getWebsites", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Referrers.getWebsites failed: {:?}", result);
}

#[tokio::test]
async fn test_referrers_get_socials() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Referrers", "getSocials", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Referrers.getSocials failed: {:?}", result);
}

#[tokio::test]
async fn test_referrers_get_campaigns() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Referrers", "getCampaigns", client.params(&config.site_id))
        .await;

    assert!(
        result.is_ok(),
        "Referrers.getCampaigns failed: {:?}",
        result
    );
}

// ============================================================================
// User Country Tests
// ============================================================================

#[tokio::test]
async fn test_user_country_get_country() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("UserCountry", "getCountry", client.params(&config.site_id))
        .await;

    assert!(
        result.is_ok(),
        "UserCountry.getCountry failed: {:?}",
        result
    );
    println!(
        "Countries: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_user_country_get_continent() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "UserCountry",
            "getContinent",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "UserCountry.getContinent failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_user_country_get_region() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("UserCountry", "getRegion", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "UserCountry.getRegion failed: {:?}", result);
}

#[tokio::test]
async fn test_user_country_get_city() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("UserCountry", "getCity", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "UserCountry.getCity failed: {:?}", result);
}

// ============================================================================
// Devices Detection Tests
// ============================================================================

#[tokio::test]
async fn test_devices_detection_get_type() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getType",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getType failed: {:?}",
        result
    );
    println!(
        "Device types: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_devices_detection_get_brand() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getBrand",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getBrand failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_devices_detection_get_model() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getModel",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getModel failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_devices_detection_get_browsers() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getBrowsers",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getBrowsers failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_devices_detection_get_browser_versions() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getBrowserVersions",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getBrowserVersions failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_devices_detection_get_os_families() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getOsFamilies",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getOsFamilies failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_devices_detection_get_os_versions() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "DevicesDetection",
            "getOsVersions",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "DevicesDetection.getOsVersions failed: {:?}",
        result
    );
}

// ============================================================================
// Resolution Tests
// ============================================================================

#[tokio::test]
async fn test_resolution_get_resolution() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Resolution",
            "getResolution",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Resolution.getResolution failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_resolution_get_configuration() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Resolution",
            "getConfiguration",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Resolution.getConfiguration failed: {:?}",
        result
    );
}

// ============================================================================
// User Language Tests
// ============================================================================

#[tokio::test]
async fn test_user_language_get_language() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "UserLanguage",
            "getLanguage",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "UserLanguage.getLanguage failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_user_language_get_language_code() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "UserLanguage",
            "getLanguageCode",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "UserLanguage.getLanguageCode failed: {:?}",
        result
    );
}

// ============================================================================
// Visitor Interest Tests
// ============================================================================

#[tokio::test]
async fn test_visitor_interest_get_number_of_visits_per_page() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "VisitorInterest",
            "getNumberOfVisitsPerPage",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "VisitorInterest.getNumberOfVisitsPerPage failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_visitor_interest_get_number_of_visits_per_visit_duration() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "VisitorInterest",
            "getNumberOfVisitsPerVisitDuration",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "VisitorInterest.getNumberOfVisitsPerVisitDuration failed: {:?}",
        result
    );
}

// ============================================================================
// Visit Time Tests
// ============================================================================

#[tokio::test]
async fn test_visit_time_get_visit_information_per_server_time() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "VisitTime",
            "getVisitInformationPerServerTime",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "VisitTime.getVisitInformationPerServerTime failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_visit_time_get_visit_information_per_local_time() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "VisitTime",
            "getVisitInformationPerLocalTime",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "VisitTime.getVisitInformationPerLocalTime failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_visit_time_get_by_day_of_week() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "VisitTime",
            "getByDayOfWeek",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "VisitTime.getByDayOfWeek failed: {:?}",
        result
    );
}

// ============================================================================
// Visit Frequency Tests
// ============================================================================

#[tokio::test]
async fn test_visit_frequency_get() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("VisitFrequency", "get", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "VisitFrequency.get failed: {:?}", result);
    println!(
        "Visit frequency: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

// ============================================================================
// Goals Tests
// ============================================================================

#[tokio::test]
async fn test_goals_get() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Goals", "get", client.params(&config.site_id))
        .await;

    // Goals.get might return empty or error if no goals are configured
    // We accept both success and specific "no goals" error
    match result {
        Ok(goals) => {
            println!("Goals: {}", serde_json::to_string_pretty(&goals).unwrap());
        }
        Err(e) if e.contains("No goal") || e.contains("no goal") => {
            println!("No goals configured for this site (expected)");
        }
        Err(e) => {
            panic!("Goals.get failed unexpectedly: {}", e);
        }
    }
}

// ============================================================================
// Events Tests
// ============================================================================

#[tokio::test]
async fn test_events_get_category() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Events", "getCategory", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Events.getCategory failed: {:?}", result);
}

#[tokio::test]
async fn test_events_get_action() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Events", "getAction", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Events.getAction failed: {:?}", result);
}

#[tokio::test]
async fn test_events_get_name() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("Events", "getName", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "Events.getName failed: {:?}", result);
}

// ============================================================================
// Contents Tests
// ============================================================================

#[tokio::test]
async fn test_contents_get_content_names() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Contents",
            "getContentNames",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Contents.getContentNames failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_contents_get_content_pieces() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call(
            "Contents",
            "getContentPieces",
            client.params(&config.site_id),
        )
        .await;

    assert!(
        result.is_ok(),
        "Contents.getContentPieces failed: {:?}",
        result
    );
}

// ============================================================================
// Page Performance Tests
// ============================================================================

#[tokio::test]
async fn test_page_performance_get() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("PagePerformance", "get", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "PagePerformance.get failed: {:?}", result);
}

// ============================================================================
// MultiSites Tests
// ============================================================================

#[tokio::test]
async fn test_multi_sites_get_all() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = HashMap::new();
    params.insert("period".to_string(), "day".to_string());
    params.insert("date".to_string(), "today".to_string());

    let result = client.call("MultiSites", "getAll", params).await;

    assert!(result.is_ok(), "MultiSites.getAll failed: {:?}", result);
    println!(
        "All sites: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_multi_sites_get_one() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("MultiSites", "getOne", client.params(&config.site_id))
        .await;

    assert!(result.is_ok(), "MultiSites.getOne failed: {:?}", result);
}

// ============================================================================
// Sites Manager Tests
// ============================================================================

#[tokio::test]
async fn test_sites_manager_get_site_from_id() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = HashMap::new();
    params.insert("idSite".to_string(), config.site_id.clone());

    let result = client.call("SitesManager", "getSiteFromId", params).await;

    assert!(
        result.is_ok(),
        "SitesManager.getSiteFromId failed: {:?}",
        result
    );
    let site = result.unwrap();
    println!("Site: {}", serde_json::to_string_pretty(&site).unwrap());
}

#[tokio::test]
async fn test_sites_manager_get_all_sites() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let result = client
        .call("SitesManager", "getAllSites", HashMap::new())
        .await;

    assert!(
        result.is_ok(),
        "SitesManager.getAllSites failed: {:?}",
        result
    );
}

// ============================================================================
// API Reference Tests (for introspection)
// ============================================================================

#[tokio::test]
async fn test_api_get_report_metadata() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = HashMap::new();
    params.insert("idSites".to_string(), config.site_id.clone());

    let result = client.call("API", "getReportMetadata", params).await;

    assert!(result.is_ok(), "API.getReportMetadata failed: {:?}", result);
    let metadata = result.unwrap();
    assert!(
        metadata.is_array(),
        "Expected array of report metadata, got: {:?}",
        metadata
    );
    let arr = metadata.as_array().unwrap();
    println!("Found {} report methods", arr.len());
    assert!(!arr.is_empty(), "Expected at least one report method");
}

// ============================================================================
// Date Range Tests
// ============================================================================

#[tokio::test]
async fn test_visits_summary_with_date_range() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = HashMap::new();
    params.insert("idSite".to_string(), config.site_id.clone());
    params.insert("period".to_string(), "range".to_string());
    params.insert("date".to_string(), "last7".to_string());

    let result = client.call("VisitsSummary", "get", params).await;

    assert!(
        result.is_ok(),
        "VisitsSummary.get with date range failed: {:?}",
        result
    );
    println!(
        "Last 7 days visits: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

#[tokio::test]
async fn test_visits_summary_with_month_period() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = HashMap::new();
    params.insert("idSite".to_string(), config.site_id.clone());
    params.insert("period".to_string(), "month".to_string());
    params.insert("date".to_string(), "today".to_string());

    let result = client.call("VisitsSummary", "get", params).await;

    assert!(
        result.is_ok(),
        "VisitsSummary.get with month period failed: {:?}",
        result
    );
    println!(
        "This month visits: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

// ============================================================================
// Segment Tests
// ============================================================================

#[tokio::test]
async fn test_visits_summary_with_segment() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = client.params(&config.site_id);
    // Segment: desktop devices only
    params.insert("segment".to_string(), "deviceType==desktop".to_string());

    let result = client.call("VisitsSummary", "get", params).await;

    assert!(
        result.is_ok(),
        "VisitsSummary.get with segment failed: {:?}",
        result
    );
    println!(
        "Desktop visits: {}",
        serde_json::to_string_pretty(&result.unwrap()).unwrap()
    );
}

// ============================================================================
// Filter Tests
// ============================================================================

#[tokio::test]
async fn test_actions_get_page_urls_with_filter_limit() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = client.params(&config.site_id);
    params.insert("filter_limit".to_string(), "5".to_string());

    let result = client.call("Actions", "getPageUrls", params).await;

    assert!(
        result.is_ok(),
        "Actions.getPageUrls with filter_limit failed: {:?}",
        result
    );

    let pages = result.unwrap();
    if let Some(arr) = pages.as_array() {
        assert!(
            arr.len() <= 5,
            "Expected at most 5 results, got: {}",
            arr.len()
        );
    }
}

#[tokio::test]
async fn test_referrers_get_websites_with_expanded() {
    let config = require_env!();
    let client = TestMatomoClient::new(&config.url, &config.token);

    let mut params = client.params(&config.site_id);
    params.insert("expanded".to_string(), "1".to_string());

    let result = client.call("Referrers", "getWebsites", params).await;

    assert!(
        result.is_ok(),
        "Referrers.getWebsites with expanded failed: {:?}",
        result
    );
}
