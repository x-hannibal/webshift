//! SerpAPI multi-engine search backend.
//!
//! SerpAPI acts as a proxy for multiple search engines (Google, Bing, DuckDuckGo,
//! Yandex, Yahoo). The `engine` config key selects which engine.
//! Requires WEBGATE_SERPAPI_API_KEY.

use super::{SearchBackend, SearchResult};
use crate::config::SerpapiConfig;

#[derive(Debug)]
pub struct SerpapiBackend {
    config: SerpapiConfig,
    base_url: String,
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
            base_url: "https://serpapi.com".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        })
    }
}

#[cfg(test)]
impl SerpapiBackend {
    fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
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

        let url = format!("{}/search", self.base_url);
        let resp = self
            .client
            .get(&url)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config(api_key: &str) -> SerpapiConfig {
        SerpapiConfig {
            api_key: api_key.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn serpapi_new_empty_api_key_returns_error() {
        let result = SerpapiBackend::new(&test_config(""));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("WEBGATE_SERPAPI_API_KEY"));
    }

    #[tokio::test]
    async fn serpapi_search_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "organic_results": [
                {"title": "Rust Lang", "link": "https://rust-lang.org", "snippet": "Systems programming"},
                {"title": "Tokio", "link": "https://tokio.rs", "snippet": "Async runtime for Rust"},
                {"title": "Serde", "link": "https://serde.rs", "snippet": "Serialization framework"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "rust"))
            .and(query_param("engine", "google"))
            .and(query_param("api_key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = SerpapiBackend::new(&test_config("test-key"))
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
    async fn serpapi_search_empty_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"organic_results": []});

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = SerpapiBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn serpapi_search_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let backend = SerpapiBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn serpapi_search_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "organic_results": [
                {"title": "Rust IT", "link": "https://rust-lang.org/it", "snippet": "Linguaggio di sistema"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("hl", "it"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = SerpapiBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("rust", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust IT");
    }
}
