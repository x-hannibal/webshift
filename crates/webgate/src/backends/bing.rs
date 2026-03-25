//! Bing Web Search API backend.
//!
//! Requires a Microsoft Azure API key (`WEBGATE_BING_API_KEY`).
//! Free tier: 1,000 queries/month (S1 tier).
//! Create a key at <https://www.microsoft.com/en-us/bing/apis/bing-web-search-api>.

use super::{SearchBackend, SearchResult};
use crate::config::BingConfig;

#[derive(Debug)]
pub struct BingBackend {
    api_key: String,
    market: String,
    base_url: String,
    client: reqwest::Client,
}

impl BingBackend {
    pub fn new(config: &BingConfig) -> Result<Self, crate::WebgateError> {
        if config.api_key.is_empty() {
            return Err(crate::WebgateError::Backend(
                "Bing Web Search requires WEBGATE_BING_API_KEY to be set".into(),
            ));
        }
        Ok(Self {
            api_key: config.api_key.clone(),
            market: config.market.clone(),
            base_url: "https://api.bing.microsoft.com".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        })
    }
}

#[cfg(test)]
impl BingBackend {
    fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }
}

fn jstr<'a>(val: &'a serde_json::Value, key: &str) -> &'a str {
    val.get(key).and_then(serde_json::Value::as_str).unwrap_or("")
}

#[async_trait::async_trait]
impl SearchBackend for BingBackend {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError> {
        // Bing supports up to 50 results per request
        let count = num_results.min(50);
        let mkt = if let Some(lang) = lang {
            // Convert bare language code to Bing market if possible
            format!("{lang}-{}", lang.to_uppercase())
        } else {
            self.market.clone()
        };

        let params = vec![
            ("q", query.to_string()),
            ("count", count.to_string()),
            ("mkt", mkt),
            ("safeSearch", "Moderate".to_string()),
        ];

        let url = format!("{}/v7.0/search", self.base_url);
        let resp = self
            .client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .query(&params)
            .send()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("bing request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebgateError::Backend(format!(
                "bing HTTP {status}"
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::WebgateError::Backend(format!("bing parse error: {e}")))?;

        let empty = vec![];
        let items = data
            .get("webPages")
            .and_then(|w| w.get("value"))
            .and_then(serde_json::Value::as_array)
            .unwrap_or(&empty);

        let mut results = Vec::new();
        for item in items {
            if results.len() >= num_results {
                break;
            }
            results.push(SearchResult {
                title: jstr(item, "name").to_string(),
                url: jstr(item, "url").to_string(),
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

    fn test_config(api_key: &str) -> BingConfig {
        BingConfig {
            api_key: api_key.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn bing_new_empty_api_key_returns_error() {
        let result = BingBackend::new(&test_config(""));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("WEBGATE_BING_API_KEY"));
    }

    #[tokio::test]
    async fn bing_search_parses_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "webPages": {
                "value": [
                    {"name": "Rust Lang", "url": "https://rust-lang.org", "snippet": "Systems programming"},
                    {"name": "Tokio", "url": "https://tokio.rs", "snippet": "Async runtime for Rust"},
                    {"name": "Serde", "url": "https://serde.rs", "snippet": "Serialization framework"},
                ]
            }
        });

        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .and(query_param("q", "rust"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = BingBackend::new(&test_config("test-key"))
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
    async fn bing_search_caps_at_50_results() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .and(query_param("count", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"webPages": {"value": []}})))
            .mount(&mock_server)
            .await;

        let backend = BingBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("rust", 100, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn bing_search_missing_webpages_key() {
        let mock_server = MockServer::start().await;

        // Response has no "webPages" key at all
        let body = serde_json::json!({"_type": "SearchResponse"});

        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = BingBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn bing_search_empty_results() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"webPages": {"value": []}});

        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = BingBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("noresults", 5, None).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn bing_search_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let backend = BingBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let result = backend.search("test", 5, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn bing_search_with_lang_param() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "webPages": {
                "value": [
                    {"name": "Rust IT", "url": "https://rust-lang.org/it", "snippet": "Linguaggio di sistema"},
                ]
            }
        });

        // lang "it" should produce market "it-IT"
        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .and(query_param("mkt", "it-IT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let backend = BingBackend::new(&test_config("test-key"))
            .unwrap()
            .with_base_url(mock_server.uri());
        let results = backend.search("rust", 10, Some("it")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust IT");
    }
}
