//! Google Custom Search API backend.
//!
//! Requires a Google API key (`WEBGATE_GOOGLE_API_KEY`) and a Custom Search
//! Engine ID (`WEBGATE_GOOGLE_CX`). Free tier: 100 queries/day.
//! Create a CSE at <https://programmablesearchengine.google.com/>.

use super::{SearchBackend, SearchResult};
use crate::config::GoogleConfig;

#[derive(Debug)]
pub struct GoogleBackend {
    api_key: String,
    cx: String,
    base_url: String,
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
            base_url: "https://www.googleapis.com".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        })
    }
}

#[cfg(test)]
impl GoogleBackend {
    fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
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

        let url = format!("{}/customsearch/v1", self.base_url);
        let resp = self
            .client
            .get(&url)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config(api_key: &str, cx: &str) -> GoogleConfig {
        GoogleConfig {
            api_key: api_key.to_string(),
            cx: cx.to_string(),
        }
    }

    #[test]
    fn google_new_empty_api_key_returns_error() {
        let result = GoogleBackend::new(&test_config("", "my-cx"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("WEBGATE_GOOGLE_API_KEY"));
    }

    #[test]
    fn google_new_empty_cx_returns_error() {
        let result = GoogleBackend::new(&test_config("my-key", ""));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("WEBGATE_GOOGLE_CX"));
    }

    #[tokio::test]
    async fn google_search_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "items": [
                {"title": "Rust Lang", "link": "https://rust-lang.org", "snippet": "Systems programming"},
                {"title": "Tokio", "link": "https://tokio.rs", "snippet": "Async runtime for Rust"},
                {"title": "Serde", "link": "https://serde.rs", "snippet": "Serialization framework"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .and(query_param("q", "rust"))
            .and(query_param("key", "test-key"))
            .and(query_param("cx", "test-cx"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = GoogleBackend::new(&test_config("test-key", "test-cx"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("rust", 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].snippet, "Systems programming");
        assert_eq!(results[1].title, "Tokio");
    }

    #[tokio::test]
    async fn google_search_caps_at_10_results() {
        let mock_server = MockServer::start().await;

        // Request 20 results but Google CSE caps at 10
        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .and(query_param("num", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": []})))
            .mount(&mock_server)
            .await;

        let backend = GoogleBackend::new(&test_config("test-key", "test-cx"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("rust", 20, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn google_search_missing_items_key() {
        let mock_server = MockServer::start().await;

        // Response has no "items" key at all
        let body = serde_json::json!({"searchInformation": {"totalResults": "0"}});

        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = GoogleBackend::new(&test_config("test-key", "test-cx"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn google_search_empty_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"items": []});

        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = GoogleBackend::new(&test_config("test-key", "test-cx"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn google_search_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let backend = GoogleBackend::new(&test_config("test-key", "test-cx"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("403"));
    }

    #[tokio::test]
    async fn google_search_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "items": [
                {"title": "Rust IT", "link": "https://rust-lang.org/it", "snippet": "Linguaggio di sistema"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .and(query_param("lr", "lang_it"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = GoogleBackend::new(&test_config("test-key", "test-cx"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("rust", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust IT");
    }
}
