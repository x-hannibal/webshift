//! Async OpenAI-compatible chat completion client (reqwest, no SDK dependency).
//!
//! Port of `../mcp-webshift/src/mcp_webshift/llm/client.py`.

use crate::config::LlmConfig;

/// Async client for any OpenAI-compatible `/v1/chat/completions` endpoint.
///
/// Covers: OpenAI, Ollama, LM Studio, vLLM, Together AI, Groq, and any provider
/// that speaks the OpenAI chat completions protocol.
pub struct LlmClient {
    config: LlmConfig,
    client: reqwest::Client,
}

/// A single chat message in OpenAI format.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

impl LlmClient {
    /// Create a new client from config.
    pub fn new(config: &LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout))
            .build()
            .expect("failed to build LLM HTTP client");
        Self {
            config: config.clone(),
            client,
        }
    }

    /// Send a chat completion request and return the response text.
    ///
    /// Returns `WebshiftError::Llm` if LLM is not enabled or on API errors.
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        temperature: f32,
    ) -> Result<String, crate::WebshiftError> {
        if !self.config.enabled {
            return Err(crate::WebshiftError::Llm(
                "LLM client is not enabled (set llm.enabled = true in config)".into(),
            ));
        }

        #[derive(serde::Serialize)]
        struct Payload<'a> {
            model: &'a str,
            messages: &'a [ChatMessage],
            temperature: f32,
        }

        let payload = Payload {
            model: &self.config.model,
            messages,
            temperature,
        };

        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let mut req = self.client.post(&url).json(&payload);
        if !self.config.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.config.api_key));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| crate::WebshiftError::Llm(format!("LLM request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(crate::WebshiftError::Llm(format!("LLM API error HTTP {status}")));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::WebshiftError::Llm(format!("LLM response parse error: {e}")))?;

        let content = data
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                crate::WebshiftError::Llm(format!(
                    "Unexpected LLM API response (no choices): {data}"
                ))
            })?;

        Ok(content.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_config(base_url: &str) -> LlmConfig {
        LlmConfig {
            enabled: true,
            base_url: base_url.to_string(),
            api_key: String::new(),
            model: "test-model".to_string(),
            timeout: 5,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn chat_returns_content() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "Hello, world!"}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = make_config(&format!("{}/v1", mock_server.uri()));
        let client = LlmClient::new(&config);
        let result = client
            .chat(&[ChatMessage::user("say hello")], 0.0)
            .await
            .unwrap();

        assert_eq!(result, "Hello, world!");
    }

    #[tokio::test]
    async fn chat_returns_error_when_disabled() {
        let config = LlmConfig {
            enabled: false,
            ..Default::default()
        };
        let client = LlmClient::new(&config);
        let result = client
            .chat(&[ChatMessage::user("hello")], 0.0)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not enabled"));
    }

    #[tokio::test]
    async fn chat_handles_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = make_config(&format!("{}/v1", mock_server.uri()));
        let client = LlmClient::new(&config);
        let result = client
            .chat(&[ChatMessage::user("hello")], 0.0)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    #[tokio::test]
    async fn chat_no_choices_returns_error() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({"id": "x", "choices": []});

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let config = make_config(&format!("{}/v1", mock_server.uri()));
        let client = LlmClient::new(&config);
        let result = client
            .chat(&[ChatMessage::user("hello")], 0.0)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no choices"));
    }

    #[tokio::test]
    async fn chat_invalid_json_returns_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&mock_server)
            .await;

        let config = make_config(&format!("{}/v1", mock_server.uri()));
        let client = LlmClient::new(&config);
        let result = client
            .chat(&[ChatMessage::user("hello")], 0.0)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("parse error"));
    }

    #[tokio::test]
    async fn chat_no_auth_header_when_key_empty() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "ok"}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = make_config(&format!("{}/v1", mock_server.uri()));
        // api_key is already empty from make_config
        let client = LlmClient::new(&config);
        client
            .chat(&[ChatMessage::user("hello")], 0.0)
            .await
            .unwrap();

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let auth = requests[0]
            .headers
            .get("Authorization");
        assert!(auth.is_none(), "Authorization header should not be sent when api_key is empty");
    }

    #[tokio::test]
    async fn chat_sends_api_key_header() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "choices": [{"message": {"content": "ok"}}]
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(wiremock::matchers::header("Authorization", "Bearer secret-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let mut config = make_config(&format!("{}/v1", mock_server.uri()));
        config.api_key = "secret-key".to_string();
        let client = LlmClient::new(&config);
        let result = client
            .chat(&[ChatMessage::user("hello")], 0.0)
            .await
            .unwrap();

        assert_eq!(result, "ok");
    }
}
