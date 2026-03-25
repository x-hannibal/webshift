//! SerpAPI multi-engine search backend.
//!
//! SerpAPI acts as a proxy for multiple search engines (Google, Bing, DuckDuckGo,
//! Yandex, Yahoo). The `engine` config key selects which engine.
//! Requires WEBGATE_SERPAPI_API_KEY.

use super::{SearchBackend, SearchResult};
use crate::config::SerpapiConfig;

pub struct SerpapiBackend {
    config: SerpapiConfig,
    client: reqwest::Client,
}

impl SerpapiBackend {
    pub fn new(config: &SerpapiConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "SerpAPI requires WEBGATE_SERPAPI_API_KEY to be set".into(),
            ));
        }
        Ok(Self {
            config: config.clone(),
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
impl SearchBackend for SerpapiBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        let params = vec![
            ("api_key", self.config.api_key.clone()),
            ("q", query.to_string()),
            ("engine", self.config.engine.clone()),
            ("num", num_results.min(100).to_string()),
            ("gl", self.config.gl.clone()),
            ("hl", lang.unwrap_or(&self.config.hl).to_string()),
            ("safe", self.config.safe.clone()),
        ];

        let resp = self
            .client
            .get("https://serpapi.com/search")
            .query(&params)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("serpapi request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "serpapi HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("serpapi parse error: {e}")))?;

        let empty = vec![];
        let items = data
            .get("organic_results")
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
