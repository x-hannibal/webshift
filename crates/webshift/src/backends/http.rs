//! Generic configurable HTTP backend.
//!
//! Allows wiring up any REST search API through TOML configuration alone,
//! without writing Rust code. Covers the common case of a JSON endpoint that
//! accepts a query string and returns an array of results.
//!
//! Example `webshift.toml`:
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

#[derive(Debug)]
pub struct HttpBackend {
    config: HttpBackendConfig,
    client: reqwest::Client,
}

impl HttpBackend {
    pub fn new(config: &HttpBackendConfig) -> Result<Self, crate::WebshiftError> {
        if config.url.is_empty() {
            return Err(crate::WebshiftError::Backend(
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
    ) -> Result<Vec<SearchResult>, crate::WebshiftError> {
        let cfg = &self.config;

        // Build query params
        let mut params: Vec<(String, String)> = vec![
            (cfg.query_param.clone(), query.to_string()),
        ];
        if !cfg.count_param.is_empty() {
            params.push((cfg.count_param.clone(), num_results.to_string()));
        }
        if let Some(lang) = lang && !cfg.lang_param.is_empty() {
            params.push((cfg.lang_param.clone(), lang.to_string()));
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
            .map_err(|e| crate::WebshiftError::Backend(format!("http backend request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebshiftError::Backend(format!(
                "http backend HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::WebshiftError::Backend(format!("http backend parse error: {e}")))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn base_config(url: &str) -> HttpBackendConfig {
        HttpBackendConfig {
            url: url.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn http_new_empty_url_returns_error() {
        let result = HttpBackend::new(&base_config(""));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("backends.http.url"));
    }

    #[tokio::test]
    async fn http_search_parses_results_root_array() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        // Root-level array (results_path is empty by default)
        let body = serde_json::json!([
            {"title": "Rust Lang", "url": "https://rust-lang.org", "snippet": "Systems programming"},
            {"title": "Tokio", "url": "https://tokio.rs", "snippet": "Async runtime for Rust"},
        ]);

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "rust"))
            .and(query_param("count", "5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = HttpBackend::new(&base_config(&url)).unwrap();
        let results = backend.search("rust", 5, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Systems programming");
    }

    #[tokio::test]
    async fn http_search_nested_json_path() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/api", mock_server.uri());

        let body = serde_json::json!({
            "data": {
                "items": [
                    {"title": "Nested Result", "url": "https://example.com", "snippet": "Found via path"},
                ]
            }
        });

        Mock::given(method("GET"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = HttpBackendConfig {
            url: url.clone(),
            results_path: "data.items".to_string(),
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 10, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Nested Result");
    }

    #[tokio::test]
    async fn http_search_missing_json_path_key() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/api", mock_server.uri());

        // Response doesn't contain the expected path
        let body = serde_json::json!({"other": "data"});

        Mock::given(method("GET"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = HttpBackendConfig {
            url: url.clone(),
            results_path: "data.items".to_string(),
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 10, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn http_search_custom_field_names() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([
            {"name": "Custom Title", "link": "https://example.com", "description": "Custom snippet"},
        ]);

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = HttpBackendConfig {
            url: url.clone(),
            title_field: "name".to_string(),
            url_field: "link".to_string(),
            snippet_field: "description".to_string(),
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 10, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Custom Title");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[0].snippet, "Custom snippet");
    }

    #[tokio::test]
    async fn http_search_post_method() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([
            {"title": "POST Result", "url": "https://example.com", "snippet": "Via POST"},
        ]);

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = HttpBackendConfig {
            url: url.clone(),
            method: "POST".to_string(),
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 10, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "POST Result");
    }

    #[tokio::test]
    async fn http_search_custom_headers() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([
            {"title": "Authed", "url": "https://example.com", "snippet": "With auth"},
        ]);

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(wiremock::matchers::header("Authorization", "Bearer secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer secret".to_string());

        let config = HttpBackendConfig {
            url: url.clone(),
            headers,
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 10, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Authed");
    }

    #[tokio::test]
    async fn http_search_extra_params() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([]);

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "test"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let mut extra_params = HashMap::new();
        extra_params.insert("format".to_string(), "json".to_string());

        let config = HttpBackendConfig {
            url: url.clone(),
            extra_params,
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn http_search_empty_count_param_omits_count() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([]);

        // When count_param is empty, only "q" should be sent
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = HttpBackendConfig {
            url: url.clone(),
            count_param: String::new(),
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn http_search_empty_results() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([]);

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = HttpBackend::new(&base_config(&url)).unwrap();
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn http_search_http_error() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(502))
            .mount(&mock_server)
            .await;

        let backend = HttpBackend::new(&base_config(&url)).unwrap();
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("502"));
    }

    #[tokio::test]
    async fn http_search_with_lang_param() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([
            {"title": "Italian", "url": "https://example.it", "snippet": "Risultato"},
        ]);

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("lang", "it"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = HttpBackendConfig {
            url: url.clone(),
            lang_param: "lang".to_string(),
            ..Default::default()
        };
        let backend = HttpBackend::new(&config).unwrap();
        let results = backend.search("test", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Italian");
    }

    #[tokio::test]
    async fn http_search_empty_lang_param_omits_lang() {
        let mock_server = MockServer::start().await;
        let url = format!("{}/search", mock_server.uri());

        let body = serde_json::json!([]);

        // lang_param is empty by default, so even if lang is provided, no param is sent
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = HttpBackend::new(&base_config(&url)).unwrap();
        // Passing lang="it" but lang_param is empty, so it should be omitted
        let results = backend.search("test", 5, Some("it")).await.unwrap();

        assert!(results.is_empty());
    }
}
