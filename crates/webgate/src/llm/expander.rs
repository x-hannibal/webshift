//! Query expansion: given one query, generate N complementary queries via LLM.
//!
//! Port of `../mcp-webgate/src/mcp_webgate/llm/expander.py`.

use super::client::{ChatMessage, LlmClient};

/// Generate up to `n - 1` complementary queries and prepend the original.
///
/// Returns `[query]` on any error — the pipeline always has at least one query.
///
/// # Arguments
/// * `query` - The original search query.
/// * `n` - Target total number of queries (original + generated).
/// * `client` - Configured `LlmClient` instance.
pub async fn expand_queries(query: &str, n: usize, client: &LlmClient) -> Vec<String> {
    if n <= 1 {
        return vec![query.to_string()];
    }

    let prompt = format!(
        "Generate up to {} complementary search queries for the following topic. \
         Each query should approach the topic from a different angle or add specificity. \
         Output only a JSON array of strings, no explanation, no markdown.\n\nQuery: {}",
        n - 1,
        query
    );

    match client.chat(&[ChatMessage::user(prompt)], 0.0).await {
        Ok(text) => {
            let text = strip_markdown_fences(text.trim());
            match serde_json::from_str::<Vec<serde_json::Value>>(text) {
                Ok(arr) if !arr.is_empty() => {
                    let variants: Vec<String> = arr
                        .into_iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .take(n - 1)
                        .collect();
                    let mut result = vec![query.to_string()];
                    result.extend(variants);
                    result
                }
                _ => vec![query.to_string()],
            }
        }
        Err(_) => vec![query.to_string()],
    }
}

/// Strip leading/trailing markdown code fences (e.g. ```json ... ```).
fn strip_markdown_fences(text: &str) -> &str {
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text);
    text.trim()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_client(base_url: &str) -> LlmClient {
        let config = LlmConfig {
            enabled: true,
            base_url: base_url.to_string(),
            model: "test".to_string(),
            timeout: 5,
            ..Default::default()
        };
        LlmClient::new(&config)
    }

    #[tokio::test]
    async fn expand_queries_returns_original_plus_variants() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "[\"rust async patterns\", \"tokio runtime guide\"]"}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let result = expand_queries("rust programming", 3, &client).await;

        assert_eq!(result[0], "rust programming");
        assert!(result.len() >= 2);
    }

    #[tokio::test]
    async fn expand_queries_n1_returns_original() {
        // n=1 should skip LLM call entirely
        let mock_server = MockServer::start().await;
        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let result = expand_queries("test query", 1, &client).await;
        assert_eq!(result, vec!["test query"]);
    }

    #[tokio::test]
    async fn expand_queries_fallback_on_llm_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let result = expand_queries("test", 3, &client).await;

        // Falls back to original query only
        assert_eq!(result, vec!["test"]);
    }

    #[tokio::test]
    async fn expand_queries_non_array_json_fallback() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "\"not an array\""}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let result = expand_queries("test query", 3, &client).await;

        // Should fall back to original query only
        assert_eq!(result, vec!["test query"]);
    }

    #[tokio::test]
    async fn expand_queries_respects_n_cap() {
        let mock_server = MockServer::start().await;

        // LLM returns 10 variants, but n=3 means original + 2 variants max
        let variants: Vec<String> = (1..=10).map(|i| format!("variant {i}")).collect();
        let body = serde_json::json!({
            "choices": [{"message": {"content": serde_json::to_string(&variants).unwrap()}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let result = expand_queries("original", 3, &client).await;

        assert_eq!(result.len(), 3, "should be original + 2 variants");
        assert_eq!(result[0], "original");
        assert_eq!(result[1], "variant 1");
        assert_eq!(result[2], "variant 2");
    }

    #[tokio::test]
    async fn expand_queries_strips_markdown_fences() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "```json\n[\"variant one\"]\n```"}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let result = expand_queries("test", 3, &client).await;

        assert_eq!(result[0], "test");
        assert_eq!(result[1], "variant one");
    }
}
