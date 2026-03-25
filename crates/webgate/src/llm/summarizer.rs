//! Results summarization: Markdown report with inline citations via LLM.
//!
//! Port of `../mcp-webgate/src/mcp_webgate/llm/summarizer.py`.

use super::client::{ChatMessage, LlmClient};
use crate::Source;

/// Summarize search results into a concise Markdown answer with inline citations.
///
/// Sources have already been cleaned and truncated by the query pipeline
/// (bounded by `max_result_length` / `max_query_budget`), so no additional
/// input truncation is needed here.
///
/// # Arguments
/// * `queries` - Original query string(s).
/// * `sources` - Sources with `id`, `title`, `url`, `content`.
/// * `client` - Configured `LlmClient` instance.
/// * `max_words` - Target word count for the summary (prompt guideline).
pub async fn summarize_results(
    queries: &[String],
    sources: &[Source],
    client: &LlmClient,
    max_words: usize,
) -> Result<String, crate::WebgateError> {
    let query_str = queries.join(" | ");

    let context: String = sources
        .iter()
        .map(|s| {
            format!(
                "[{}] {}\n{}\n{}\n",
                s.id,
                s.title,
                s.url,
                s.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "You are a research assistant. Based on the following search results for the query \
         \"{query_str}\", write a detailed report in Markdown (aim for at most {max_words} \
         words). Cite sources using their bracketed IDs like [1], [2], etc. \
         Do not add commentary about the sources themselves, and only include information \
         contained in the provided search results.\n\nSearch results:\n{context}"
    );

    client.chat(&[ChatMessage::user(prompt)], 0.0).await
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

    fn make_source(id: usize, title: &str, url: &str, content: &str) -> Source {
        Source {
            id,
            title: title.to_string(),
            url: url.to_string(),
            snippet: None,
            content: content.to_string(),
            truncated: false,
        }
    }

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
    async fn summarize_returns_markdown() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "## Summary\n\nRust is great [1]."}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let sources = vec![make_source(
            1,
            "Rust Lang",
            "https://rust-lang.org",
            "Rust is a systems language.",
        )];
        let result = summarize_results(
            &["rust programming".to_string()],
            &sources,
            &client,
            500,
        )
        .await
        .unwrap();

        assert!(result.contains("Summary"));
        assert!(result.contains("[1]"));
    }

    #[tokio::test]
    async fn summarize_empty_sources() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "No results found."}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let sources: Vec<Source> = vec![];
        let result = summarize_results(
            &["test".to_string()],
            &sources,
            &client,
            500,
        )
        .await;

        // Should not panic, should make LLM call and return ok
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn summarize_includes_max_words_in_prompt() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "Summary text."}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let sources = vec![make_source(1, "Test", "https://test.com", "content")];
        summarize_results(
            &["test".to_string()],
            &sources,
            &client,
            500,
        )
        .await
        .unwrap();

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body_str = String::from_utf8_lossy(&requests[0].body);
        assert!(body_str.contains("500"), "prompt should include max_words value '500'");
    }

    #[tokio::test]
    async fn summarize_propagates_llm_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = make_client(&format!("{}/v1", mock_server.uri()));
        let sources = vec![make_source(1, "Test", "https://test.com", "content")];
        let result = summarize_results(
            &["test".to_string()],
            &sources,
            &client,
            500,
        )
        .await;

        assert!(result.is_err());
    }
}
