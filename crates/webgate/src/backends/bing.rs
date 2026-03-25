//! Bing Web Search API backend.
//!
//! Requires a Microsoft Azure API key (`WEBGATE_BING_API_KEY`).
//! Free tier: 1,000 queries/month (S1 tier).
//! Create a key at <https://www.microsoft.com/en-us/bing/apis/bing-web-search-api>.

use super::{SearchBackend, SearchResult};
use crate::config::BingConfig;

pub struct BingBackend {
    api_key: String,
    market: String,
    client: reqwest::Client,
}

impl BingBackend {
    pub fn new(config: &BingConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Bing Web Search requires WEBGATE_BING_API_KEY to be set".into(),
            ));
        }
        Ok(Self {
            api_key: config.api_key.clone(),
            market: config.market.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        })
    }
}

fn jstr<'a>(val: &'a serde_json::Value, key: &str) -> &'a str {
    val.get(key).and_then(serde_json::Value::as_str).unwrap_or("")
}

#[async_trait::async_trait]
impl SearchBackend for BingBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        // Bing supports up to 50 results per request
        let count = num_results.min(50);
        let mkt = if let Some(lang) = lang {
            // Convert bare language code to Bing market if possible
            format!("{lang}-{}", lang.to_uppercase())
        } else {
            self.market.clone()
        };

        let params = vec![
            ("q", query.to_string()),
            ("count", count.to_string()),
            ("mkt", mkt),
            ("safeSearch", "Moderate".to_string()),
        ];

        let resp = self
            .client
            .get("https://api.bing.microsoft.com/v7.0/search")
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .query(&params)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("bing request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "bing HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("bing parse error: {e}")))?;

        let empty = vec![];
        let items = data
            .get("webPages")
            .and_then(|w| w.get("value"))
            .and_then(serde_json::Value::as_array)
            .unwrap_or(&empty);

        let mut results = Vec::new();
        for item in items {
            if results.len() >= num_results {
                break;
            }
            results.push(SearchResult {
                title: jstr(item, "name").to_string(),
                url: jstr(item, "url").to_string(),
                snippet: jstr(item, "snippet").to_string(),
            });
        }

        Ok(results)
    }
}
