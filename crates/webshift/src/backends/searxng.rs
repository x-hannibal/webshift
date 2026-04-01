//! SearXNG search backend.
//!
//! Queries a self-hosted SearXNG instance. No API key required.

use super::{SearchBackend, SearchResult};
use crate::config::SearxngConfig;

#[derive(Debug)]
pub struct SearxngBackend {
    base_url: String,
    client: reqwest::Client,
}

impl SearxngBackend {
    pub fn new(config: &SearxngConfig) -> Self {
        Self {
            base_url: config.url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

/// Extract a string field from a JSON value, defaulting to "".
fn jstr<'a>(val: &'a serde_json::Value, key: &str) -> &'a str {
    val.get(key).and_then(serde_json::Value::as_str).unwrap_or("")
}

#[async_trait::async_trait]
impl SearchBackend for SearxngBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebshiftError> {
        let mut params = vec![
            ("q", query.to_string()),
            ("format", "json".to_string()),
            ("pageno", "1".to_string()),
        ];
        if let Some(lang) = lang {
            params.push(("language", lang.to_string()));
        }

        let url = format!("{}/search", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| crate::WebshiftError::Backend(format!("searxng request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebshiftError::Backend(format!(
                "searxng HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| crate::WebshiftError::Backend(format!("searxng parse error: {e}")))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn searxng_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {"title": "Rust Lang", "url": "https://rust-lang.org", "content": "Systems programming"},
                {"title": "Tokio", "url": "https://tokio.rs", "content": "Async runtime for Rust"},
                {"title": "Serde", "url": "https://serde.rs", "content": "Serialization framework"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "rust"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = crate::config::SearxngConfig {
            url: mock_server.uri(),
        };
        let backend = SearxngBackend::new(&config);
        let results = backend.search("rust", 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Systems programming");
        assert_eq!(results[1].title, "Tokio");
    }

    #[tokio::test]
    async fn searxng_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {"title": "Rust IT", "url": "https://rust-lang.org/it", "content": "Linguaggio di sistema"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("language", "it"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = crate::config::SearxngConfig {
            url: mock_server.uri(),
        };
        let backend = SearxngBackend::new(&config);
        let results = backend.search("rust", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust IT");
    }

    #[tokio::test]
    async fn searxng_handles_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = crate::config::SearxngConfig {
            url: mock_server.uri(),
        };
        let backend = SearxngBackend::new(&config);
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn searxng_handles_empty_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"results": []});

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = crate::config::SearxngConfig {
            url: mock_server.uri(),
        };
        let backend = SearxngBackend::new(&config);
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }
}
