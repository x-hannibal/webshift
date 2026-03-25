//! Exa neural search API backend.
//!
//! Exa uses semantic/neural search by default. autoprompt is always disabled
//! because webgate handles query expansion itself.
//! Requires WEBGATE_EXA_API_KEY.

use super::{SearchBackend, SearchResult};
use crate::config::ExaConfig;

#[derive(Debug)]
pub struct ExaBackend {
    config: ExaConfig,
    base_url: String,
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
            base_url: "https://api.exa.ai".to_string(),
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

        let url = format!("{}/search", self.base_url);
        let resp = self
            .client
            .post(&url)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: create an ExaBackend pointing at the mock server.
    fn mock_backend(uri: &str) -> ExaBackend {
        let config = ExaConfig {
            api_key: "test-key".to_string(),
            num_sentences: 3,
            search_type: "neural".to_string(),
        };
        let mut backend = ExaBackend::new(&config).unwrap();
        backend.base_url = uri.to_string();
        backend
    }

    #[test]
    fn exa_new_empty_api_key_returns_error() {
        let config = ExaConfig {
            api_key: String::new(),
            num_sentences: 3,
            search_type: "neural".to_string(),
        };
        let result = ExaBackend::new(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("WEBGATE_EXA_API_KEY"));
    }

    #[tokio::test]
    async fn exa_search_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {
                    "title": "Rust Lang",
                    "url": "https://rust-lang.org",
                    "highlights": ["Systems programming language"],
                    "text": "Full page text here"
                },
                {
                    "title": "Tokio",
                    "url": "https://tokio.rs",
                    "highlights": ["Async runtime for Rust"],
                    "text": "Tokio full text"
                },
                {
                    "title": "Serde",
                    "url": "https://serde.rs",
                    "highlights": ["Serialization framework"],
                    "text": "Serde full text"
                },
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
        // Snippet should come from highlights, not text.
        assert_eq!(results[0].snippet, "Systems programming language");
        assert_eq!(results[1].title, "Tokio");
    }

    #[tokio::test]
    async fn exa_search_empty_results() {
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
    async fn exa_search_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("403"));
    }

    #[tokio::test]
    async fn exa_search_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {
                    "title": "Rust",
                    "url": "https://rust-lang.org",
                    "highlights": ["Programming"],
                },
            ]
        });

        // Exa ignores the lang parameter — it should still return results.
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
    async fn exa_search_num_results_cap() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"results": []});

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        // Request 50 — payload should contain numResults: 10 (capped).
        let results = backend.search("rust", 50, None).await.unwrap();

        assert!(results.is_empty());

        // Verify the request body contained the cap.
        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let sent: serde_json::Value =
            serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(sent["numResults"], 10);
    }

    #[tokio::test]
    async fn exa_search_highlights_first_then_text_fallback() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "results": [
                {
                    "title": "With Highlights",
                    "url": "https://example.com/a",
                    "highlights": ["Highlight snippet"],
                    "text": "Fallback text"
                },
                {
                    "title": "Without Highlights",
                    "url": "https://example.com/b",
                    "highlights": [],
                    "text": "Text fallback used"
                },
                {
                    "title": "No Highlights Key",
                    "url": "https://example.com/c",
                    "text": "Only text available"
                },
            ]
        });

        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = mock_backend(&mock_server.uri());
        let results = backend.search("test", 10, None).await.unwrap();

        assert_eq!(results.len(), 3);
        // First result: highlights present → use highlight.
        assert_eq!(results[0].snippet, "Highlight snippet");
        // Second result: empty highlights array → fall back to text.
        assert_eq!(results[1].snippet, "Text fallback used");
        // Third result: no highlights key → fall back to text.
        assert_eq!(results[2].snippet, "Only text available");
    }
}
