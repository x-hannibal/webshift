//! Brave Search API backend.
//!
//! Requires a free-tier API key (WEBGATE_BRAVE_API_KEY).

use super::{SearchBackend, SearchResult};
use crate::config::BraveConfig;

#[derive(Debug)]
pub struct BraveBackend {
    api_key: String,
    safesearch: u8,
    base_url: String,
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
            base_url: "https://api.search.brave.com".to_string(),
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

        let url = format!("{}/res/v1/web/search", self.base_url);
        let resp = self
            .client
            .get(&url)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: create a BraveBackend pointing at the mock server.
    fn mock_backend(uri: &str, safesearch: u8) -> BraveBackend {
        let config = BraveConfig {
            api_key: "test-key".to_string(),
            safesearch,
        };
        let mut backend = BraveBackend::new(&config).unwrap();
        backend.base_url = uri.to_string();
        backend
    }

    #[test]
    fn brave_new_empty_api_key_returns_error() {
        let config = BraveConfig {
            api_key: String::new(),
            safesearch: 1,
        };
        let result = BraveBackend::new(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("WEBGATE_BRAVE_API_KEY"));
    }

    #[tokio::test]
    async fn brave_search_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "web": {
                "results": [
                    {"title": "Rust Lang", "url": "https://rust-lang.org", "description": "Systems programming language"},
                    {"title": "Tokio", "url": "https://tokio.rs", "description": "Async runtime for Rust"},
                    {"title": "Serde", "url": "https://serde.rs", "description": "Serialization framework"},
                ]
            }
        });

        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .and(query_param("q", "rust"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 1);
        let results = backend.search("rust", 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Systems programming language");
        assert_eq!(results[1].title, "Tokio");
    }

    #[tokio::test]
    async fn brave_search_empty_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"web": {"results": []}});

        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 1);
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn brave_search_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 1);
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("429"));
    }

    #[tokio::test]
    async fn brave_search_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "web": {
                "results": [
                    {"title": "Rust IT", "url": "https://rust-lang.org/it", "description": "Linguaggio di sistema"},
                ]
            }
        });

        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .and(query_param("search_lang", "it"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 1);
        let results = backend.search("rust", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust IT");
    }

    #[tokio::test]
    async fn brave_search_num_results_cap() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"web": {"results": []}});

        // Request 50 results — count param should be capped to 20.
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .and(query_param("count", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 1);
        let results = backend.search("rust", 50, None).await.unwrap();

        assert!(results.is_empty()); // mock returned none, just verifying the cap was sent
    }

    #[tokio::test]
    async fn brave_search_missing_web_key() {
        let mock_server = MockServer::start().await;

        // Response has no "web" key at all.
        let body = serde_json::json!({"query": {"original": "rust"}});

        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 1);
        let results = backend.search("rust", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn brave_safesearch_mapping() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"web": {"results": []}});

        // safesearch=0 → "off"
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .and(query_param("safesearch", "off"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 0);
        backend.search("test", 5, None).await.unwrap();

        // Verify the mock was hit (expect(1) will panic on drop if not matched).
    }

    #[tokio::test]
    async fn brave_safesearch_strict() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"web": {"results": []}});

        // safesearch=2 → "strict"
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .and(query_param("safesearch", "strict"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri(), 2);
        backend.search("test", 5, None).await.unwrap();
    }
}
