//! Brave Search API backend.
//!
//! Requires a free-tier API key (WEBGATE_BRAVE_API_KEY).

use super::{SearchBackend, SearchResult};
use crate::config::BraveConfig;

pub struct BraveBackend {
    api_key: String,
    safesearch: u8,
    client: reqwest::Client,
}

impl BraveBackend {
    pub fn new(config: &BraveConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Brave Search requires WEBGATE_BRAVE_API_KEY to be set".into(),
            ));
        }
        Ok(Self {
            api_key: config.api_key.clone(),
            safesearch: config.safesearch,
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
impl SearchBackend for BraveBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        let safesearch = match self.safesearch.min(2) {
            0 => "off",
            1 => "moderate",
            _ => "strict",
        };

        let count = num_results.min(20);
        let mut params = vec![
            ("q", query.to_string()),
            ("count", count.to_string()),
            ("safesearch", safesearch.to_string()),
        ];
        if let Some(lang) = lang {
            params.push(("search_lang", lang.to_string()));
        }

        let resp = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip")
            .header("X-Subscription-Token", &self.api_key)
            .query(&params)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("brave request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "brave HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("brave parse error: {e}")))?;

        let empty = vec![];
        let items = data
            .get("web")
            .and_then(|w: &serde_json::Value| w.get("results"))
            .and_then(serde_json::Value::as_array)
            .unwrap_or(&empty);

        let mut results = Vec::new();
        for item in items {
            if results.len() >= num_results {
                break;
            }
            results.push(SearchResult {
                title: jstr(item, "title").to_string(),
                url: jstr(item, "url").to_string(),
                snippet: jstr(item, "description").to_string(),
            });
        }

        Ok(results)
    }
}
