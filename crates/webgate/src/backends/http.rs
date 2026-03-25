//! Generic configurable HTTP backend.
//!
//! Allows wiring up any REST search API through TOML configuration alone,
//! without writing Rust code. Covers the common case of a JSON endpoint that
//! accepts a query string and returns an array of results.
//!
//! Example `webgate.toml`:
//!
//! ```toml
//! [backends.http]
//! url          = "https://my-search.example.com/api/search"
//! method       = "GET"
//! query_param  = "q"
//! count_param  = "limit"
//! results_path = "data.items"     # dot-separated JSON path to results array
//! title_field  = "title"
//! url_field    = "link"
//! snippet_field = "description"
//!
//! [backends.http.headers]
//! "Authorization" = "Bearer my-secret-token"
//! ```

use std::collections::HashMap;

use super::{SearchBackend, SearchResult};
use crate::config::HttpBackendConfig;

pub struct HttpBackend {
    config: HttpBackendConfig,
    client: reqwest::Client,
}

impl HttpBackend {
    pub fn new(config: &HttpBackendConfig) -> Result<Self, crate::WebgateError> {
        if config.url.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Generic HTTP backend requires backends.http.url to be set".into(),
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

/// Walk a dot-separated path through a JSON value.
/// `"data.items"` → `value["data"]["items"]`
fn json_path<'a>(mut val: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    for key in path.split('.') {
        val = val.get(key)?;
    }
    Some(val)
}

fn jstr<'a>(val: &'a serde_json::Value, key: &str) -> &'a str {
    val.get(key).and_then(serde_json::Value::as_str).unwrap_or("")
}

#[async_trait::async_trait]
impl SearchBackend for HttpBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        let cfg = &self.config;

        // Build query params
        let mut params: Vec<(String, String)> = vec![
            (cfg.query_param.clone(), query.to_string()),
        ];
        if !cfg.count_param.is_empty() {
            params.push((cfg.count_param.clone(), num_results.to_string()));
        }
        if let Some(lang) = lang {
            if !cfg.lang_param.is_empty() {
                params.push((cfg.lang_param.clone(), lang.to_string()));
            }
        }
        // Merge extra static params from config
        for (k, v) in &cfg.extra_params {
            params.push((k.clone(), v.clone()));
        }

        let mut req = if cfg.method.eq_ignore_ascii_case("POST") {
            // For POST, send params as JSON body
            let body: HashMap<&str, &str> =
                params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            self.client.post(&cfg.url).json(&body)
        } else {
            self.client.get(&cfg.url).query(&params)
        };

        // Attach headers
        for (k, v) in &cfg.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("http backend request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "http backend HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("http backend parse error: {e}")))?;

        let empty = vec![];
        let items = if cfg.results_path.is_empty() {
            data.as_array().unwrap_or(&empty)
        } else {
            json_path(&data, &cfg.results_path)
                .and_then(serde_json::Value::as_array)
                .unwrap_or(&empty)
        };

        let mut results = Vec::new();
        for item in items {
            if results.len() >= num_results {
                break;
            }
            results.push(SearchResult {
                title: jstr(item, &cfg.title_field).to_string(),
                url: jstr(item, &cfg.url_field).to_string(),
                snippet: jstr(item, &cfg.snippet_field).to_string(),
            });
        }

        Ok(results)
    }
}
