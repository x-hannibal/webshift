//! Google Custom Search API backend.
//!
//! Requires a Google API key (`WEBGATE_GOOGLE_API_KEY`) and a Custom Search
//! Engine ID (`WEBGATE_GOOGLE_CX`). Free tier: 100 queries/day.
//! Create a CSE at <https://programmablesearchengine.google.com/>.

use super::{SearchBackend, SearchResult};
use crate::config::GoogleConfig;

pub struct GoogleBackend {
    api_key: String,
    cx: String,
    client: reqwest::Client,
}

impl GoogleBackend {
    pub fn new(config: &GoogleConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Google Custom Search requires WEBGATE_GOOGLE_API_KEY to be set".into(),
            ));
        }
        if config.cx.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Google Custom Search requires WEBGATE_GOOGLE_CX (Custom Search Engine ID)".into(),
            ));
        }
        Ok(Self {
            api_key: config.api_key.clone(),
            cx: config.cx.clone(),
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
impl SearchBackend for GoogleBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        // Google CSE max is 10 per request; we cap at 10
        let count = num_results.min(10);
        let mut params = vec![
            ("key", self.api_key.clone()),
            ("cx", self.cx.clone()),
            ("q", query.to_string()),
            ("num", count.to_string()),
        ];
        if let Some(lang) = lang {
            // lr= expects language code like "lang_en"
            params.push(("lr", format!("lang_{lang}")));
        }

        let resp = self
            .client
            .get("https://www.googleapis.com/customsearch/v1")
            .query(&params)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("google request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "google HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("google parse error: {e}")))?;

        let empty = vec![];
        let items = data
            .get("items")
            .and_then(serde_json::Value::as_array)
            .unwrap_or(&empty);

        let mut results = Vec::new();
        for item in items {
            if results.len() >= num_results {
                break;
            }
            results.push(SearchResult {
                title: jstr(item, "title").to_string(),
                url: jstr(item, "link").to_string(),
                snippet: jstr(item, "snippet").to_string(),
            });
        }

        Ok(results)
    }
}
