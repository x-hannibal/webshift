//! Tavily Search API backend.
//!
//! POST-based API. Requires a free-tier API key (WEBGATE_TAVILY_API_KEY).

use super::{SearchBackend, SearchResult};
use crate::config::TavilyConfig;

#[derive(Debug)]
pub struct TavilyBackend {
    api_key: String,
    search_depth: String,
    base_url: String,
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
            base_url: "https://api.tavily.com".to_string(),
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

        let url = format!("{}/search", self.base_url);
        let resp = self
            .client
            .post(&url)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: create a TavilyBackend pointing at the mock server.
    fn mock_backend(uri: &str) -> TavilyBackend {
        let config = TavilyConfig {
            api_key: "test-key".to_string(),
            search_depth: "basic".to_string(),
        };
        let mut backend = TavilyBackend::new(&config).unwrap();
        backend.base_url = uri.to_string();
        backend
    }

    #[test]
    fn tavily_new_empty_api_key_returns_error() {
        let config = TavilyConfig {
            api_key: String::new(),
            search_depth: "basic".to_string(),
        };
        let result = TavilyBackend::new(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("WEBGATE_TAVILY_API_KEY"));
    }

    #[tokio::test]
    async fn tavily_search_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {"title": "Rust Lang", "url": "https://rust-lang.org", "content": "Systems programming language"},
                {"title": "Tokio", "url": "https://tokio.rs", "content": "Async runtime for Rust"},
                {"title": "Serde", "url": "https://serde.rs", "content": "Serialization framework"},
            ]
        });

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        let results = backend.search("rust", 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Systems programming language");
        assert_eq!(results[1].title, "Tokio");
    }

    #[tokio::test]
    async fn tavily_search_empty_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"results": []});

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn tavily_search_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn tavily_search_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {"title": "Rust", "url": "https://rust-lang.org", "content": "Programming"},
            ]
        });

        // Tavily ignores the lang parameter — it should still return results.
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        let results = backend.search("rust", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
    }

    #[tokio::test]
    async fn tavily_search_num_results_cap() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"results": []});

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        // Request 50 — payload should contain max_results: 20 (capped).
        let results = backend.search("rust", 50, None).await.unwrap();

        assert!(results.is_empty());

        // Verify the request body contained the cap.
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let sent: serde_json::Value =
            serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(sent["max_results"], 20);
    }
}
