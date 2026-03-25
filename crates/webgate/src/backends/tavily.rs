//! Tavily Search API backend.
//!
//! POST-based API. Requires a free-tier API key (WEBGATE_TAVILY_API_KEY).

use super::{SearchBackend, SearchResult};
use crate::config::TavilyConfig;

pub struct TavilyBackend {
    api_key: String,
    search_depth: String,
    client: reqwest::Client,
}

impl TavilyBackend {
    pub fn new(config: &TavilyConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Tavily Search requires WEBGATE_TAVILY_API_KEY to be set".into(),
            ));
        }
        Ok(Self {
            api_key: config.api_key.clone(),
            search_depth: config.search_depth.clone(),
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
impl SearchBackend for TavilyBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        let _ = lang; // Tavily doesn't support language parameter

        let payload = serde_json::json!({
            "api_key": self.api_key,
            "query": query,
            "search_depth": self.search_depth,
            "max_results": num_results.min(20),
            "include_answer": false,
            "include_raw_content": false,
        });

        let resp = self
            .client
            .post("https://api.tavily.com/search")
            .json(&payload)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("tavily request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "tavily HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("tavily parse error: {e}")))?;

        let empty = vec![];
        let items = data
            .get("results")
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
                snippet: jstr(item, "content").to_string(),
            });
        }

        Ok(results)
    }
}
