//! Exa neural search API backend.
//!
//! Exa uses semantic/neural search by default. autoprompt is always disabled
//! because webgate handles query expansion itself.
//! Requires WEBGATE_EXA_API_KEY.

use super::{SearchBackend, SearchResult};
use crate::config::ExaConfig;

pub struct ExaBackend {
    config: ExaConfig,
    client: reqwest::Client,
}

impl ExaBackend {
    pub fn new(config: &ExaConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Exa Search requires WEBGATE_EXA_API_KEY to be set".into(),
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
impl SearchBackend for ExaBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        let _ = lang; // Exa doesn't support language parameter

        let payload = serde_json::json!({
            "query": query,
            "numResults": num_results.min(10),
            "useAutoprompt": false,
            "type": self.config.search_type,
            "contents": {
                "highlights": {
                    "numSentences": self.config.num_sentences,
                    "highlightsPerUrl": 1,
                },
            },
        });

        let resp = self
            .client
            .post("https://api.exa.ai/search")
            .header("x-api-key", &self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("exa request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!("exa HTTP {status}")));
        }

        let data: serde_json::Value = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("exa parse error: {e}")))?;

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
            // Prefer highlights over raw text as snippet
            let snippet = item
                .get("highlights")
                .and_then(serde_json::Value::as_array)
                .and_then(|a| a.first())
                .and_then(serde_json::Value::as_str)
                .or_else(|| item.get("text").and_then(serde_json::Value::as_str))
                .unwrap_or("");

            results.push(SearchResult {
                title: jstr(item, "title").to_string(),
                url: jstr(item, "url").to_string(),
                snippet: snippet.to_string(),
            });
        }

        Ok(results)
    }
}
