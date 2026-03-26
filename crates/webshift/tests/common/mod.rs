//! Shared helpers for integration tests.
//!
//! Loads `test.toml` from the workspace root and provides a `TestConfig`
//! struct with per-backend/LLM `enabled` flags that don't exist in the
//! library's public config types.

use serde::Deserialize;
use std::path::PathBuf;

// ── TestConfig top-level ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestConfig {
    pub server: TestServerConfig,
    pub backends: TestBackendsConfig,
    pub llm: TestLlmConfig,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            server: TestServerConfig::default(),
            backends: TestBackendsConfig::default(),
            llm: TestLlmConfig::default(),
        }
    }
}

// ── Server ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestServerConfig {
    pub max_total_results: usize,
    pub max_query_budget: usize,
    pub search_timeout: u64,
    pub language: String,
    pub adaptive_budget: webshift::config::AdaptiveBudget,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            max_total_results: 5,
            max_query_budget: 16_000,
            search_timeout: 10,
            language: "en".to_string(),
            adaptive_budget: webshift::config::AdaptiveBudget::Auto,
        }
    }
}

// ── Backends ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct TestBackendsConfig {
    #[allow(dead_code)]
    #[serde(default = "default_backend")]
    pub default: String,
    pub searxng: TestSearxng,
    pub brave: TestBrave,
    pub tavily: TestTavily,
    pub exa: TestExa,
    pub serpapi: TestSerpapi,
    pub google: TestGoogle,
    pub bing: TestBing,
    pub http: TestHttp,
}

fn default_backend() -> String {
    "searxng".into()
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestSearxng {
    pub enabled: bool,
    pub url: String,
}

impl Default for TestSearxng {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://localhost:8080".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestBrave {
    pub enabled: bool,
    pub api_key: String,
    pub safesearch: u8,
}

impl Default for TestBrave {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            safesearch: 1,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestTavily {
    pub enabled: bool,
    pub api_key: String,
    pub search_depth: String,
}

impl Default for TestTavily {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            search_depth: "basic".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestExa {
    pub enabled: bool,
    pub api_key: String,
    pub num_sentences: u32,
    #[serde(rename = "type")]
    pub search_type: String,
}

impl Default for TestExa {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            num_sentences: 3,
            search_type: "auto".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestSerpapi {
    pub enabled: bool,
    pub api_key: String,
    pub engine: String,
    pub gl: String,
    pub hl: String,
    pub safe: String,
}

impl Default for TestSerpapi {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            engine: "google".into(),
            gl: "us".into(),
            hl: "en".into(),
            safe: "off".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestGoogle {
    pub enabled: bool,
    pub api_key: String,
    pub cx: String,
}

impl Default for TestGoogle {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            cx: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestBing {
    pub enabled: bool,
    pub api_key: String,
    pub market: String,
}

impl Default for TestBing {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            market: "en-US".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestHttp {
    pub enabled: bool,
    pub url: String,
}

impl Default for TestHttp {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
        }
    }
}

// ── LLM ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestLlmConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout: u64,
    pub expansion_enabled: bool,
    pub summarization_enabled: bool,
    pub llm_rerank_enabled: bool,
    pub max_summary_words: usize,
    pub input_budget_factor: u32,
}

impl Default for TestLlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:11434/v1".into(),
            api_key: String::new(),
            model: "gemma3:27b".into(),
            timeout: 30,
            expansion_enabled: true,
            summarization_enabled: true,
            llm_rerank_enabled: false,
            max_summary_words: 0,
            input_budget_factor: 3,
        }
    }
}

// ── Loading & conversion ────────────────────────────────────────────

impl TestConfig {
    /// Load from `test.toml` at workspace root.
    pub fn load() -> Option<Self> {
        let path = find_test_toml()?;
        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    }

    /// List backend names that are enabled.
    #[allow(dead_code)]
    pub fn enabled_backends(&self) -> Vec<&str> {
        let mut v = Vec::new();
        if self.backends.searxng.enabled {
            v.push("searxng");
        }
        if self.backends.brave.enabled {
            v.push("brave");
        }
        if self.backends.tavily.enabled {
            v.push("tavily");
        }
        if self.backends.exa.enabled {
            v.push("exa");
        }
        if self.backends.serpapi.enabled {
            v.push("serpapi");
        }
        if self.backends.google.enabled {
            v.push("google");
        }
        if self.backends.bing.enabled {
            v.push("bing");
        }
        if self.backends.http.enabled {
            v.push("http");
        }
        v
    }

    /// Convert to a `webshift::Config` with the given backend as default.
    pub fn to_webshift_config(&self, backend: &str) -> webshift::Config {
        use webshift::config::*;

        Config {
            server: ServerConfig {
                max_total_results: self.server.max_total_results,
                max_query_budget: self.server.max_query_budget,
                search_timeout: self.server.search_timeout,
                language: self.server.language.clone(),
                adaptive_budget: self.server.adaptive_budget.clone(),
                ..ServerConfig::default()
            },
            backends: BackendsConfig {
                default: backend.to_string(),
                searxng: SearxngConfig {
                    url: self.backends.searxng.url.clone(),
                },
                brave: BraveConfig {
                    api_key: self.backends.brave.api_key.clone(),
                    safesearch: self.backends.brave.safesearch,
                },
                tavily: TavilyConfig {
                    api_key: self.backends.tavily.api_key.clone(),
                    search_depth: self.backends.tavily.search_depth.clone(),
                },
                exa: ExaConfig {
                    api_key: self.backends.exa.api_key.clone(),
                    num_sentences: self.backends.exa.num_sentences,
                    search_type: self.backends.exa.search_type.clone(),
                },
                serpapi: SerpapiConfig {
                    api_key: self.backends.serpapi.api_key.clone(),
                    engine: self.backends.serpapi.engine.clone(),
                    gl: self.backends.serpapi.gl.clone(),
                    hl: self.backends.serpapi.hl.clone(),
                    safe: self.backends.serpapi.safe.clone(),
                },
                google: GoogleConfig {
                    api_key: self.backends.google.api_key.clone(),
                    cx: self.backends.google.cx.clone(),
                },
                bing: BingConfig {
                    api_key: self.backends.bing.api_key.clone(),
                    market: self.backends.bing.market.clone(),
                },
                http: HttpBackendConfig {
                    url: self.backends.http.url.clone(),
                    ..HttpBackendConfig::default()
                },
            },
            llm: LlmConfig {
                enabled: self.llm.enabled,
                base_url: self.llm.base_url.clone(),
                api_key: self.llm.api_key.clone(),
                model: self.llm.model.clone(),
                timeout: self.llm.timeout,
                expansion_enabled: self.llm.expansion_enabled,
                summarization_enabled: self.llm.summarization_enabled,
                llm_rerank_enabled: self.llm.llm_rerank_enabled,
                max_summary_words: self.llm.max_summary_words,
                input_budget_factor: self.llm.input_budget_factor,
            },
        }
    }
}

fn find_test_toml() -> Option<PathBuf> {
    // In integration tests CARGO_MANIFEST_DIR points to crates/webshift.
    // Walk up to workspace root.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let mut dir = PathBuf::from(manifest);
    for _ in 0..3 {
        let candidate = dir.join("test.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}
