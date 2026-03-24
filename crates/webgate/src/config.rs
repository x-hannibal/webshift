//! Configuration system: CLI args > env vars > webgate.toml > defaults.

use serde::Deserialize;

/// Top-level configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub backends: BackendsConfig,
    pub llm: LlmConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            backends: BackendsConfig::default(),
            llm: LlmConfig::default(),
        }
    }
}

/// Server-level settings and anti-flooding caps.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub max_download_mb: u32,
    pub max_result_length: usize,
    pub search_timeout: u64,
    pub oversampling_factor: u32,
    pub auto_recovery_fetch: bool,
    pub max_total_results: usize,
    pub max_query_budget: usize,
    pub max_search_queries: usize,
    pub results_per_query: usize,
    pub blocked_domains: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub debug: bool,
    pub log_file: String,
    pub trace: bool,
    pub adaptive_budget: bool,
    pub adaptive_budget_fetch_factor: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_download_mb: 1,
            max_result_length: 8000,
            search_timeout: 8,
            oversampling_factor: 2,
            auto_recovery_fetch: false,
            max_total_results: 20,
            max_query_budget: 32000,
            max_search_queries: 5,
            results_per_query: 5,
            blocked_domains: Vec::new(),
            allowed_domains: Vec::new(),
            debug: false,
            log_file: String::new(),
            trace: false,
            adaptive_budget: false,
            adaptive_budget_fetch_factor: 3,
        }
    }
}

impl ServerConfig {
    /// Hard cap in bytes for streaming download.
    pub fn max_download_bytes(&self) -> usize {
        self.max_download_mb as usize * 1024 * 1024
    }
}

/// Backend selection and per-backend config.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BackendsConfig {
    pub default: String,
    pub searxng: SearxngConfig,
    pub brave: BraveConfig,
    pub tavily: TavilyConfig,
    pub exa: ExaConfig,
    pub serpapi: SerpapiConfig,
}

impl Default for BackendsConfig {
    fn default() -> Self {
        Self {
            default: "searxng".to_string(),
            searxng: SearxngConfig::default(),
            brave: BraveConfig::default(),
            tavily: TavilyConfig::default(),
            exa: ExaConfig::default(),
            serpapi: SerpapiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SearxngConfig {
    pub url: String,
}

impl Default for SearxngConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8080".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BraveConfig {
    pub api_key: String,
    pub safesearch: u8,
}

impl Default for BraveConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            safesearch: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TavilyConfig {
    pub api_key: String,
    pub search_depth: String,
}

impl Default for TavilyConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            search_depth: "basic".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ExaConfig {
    pub api_key: String,
    pub num_sentences: u32,
    #[serde(rename = "type")]
    pub search_type: String,
}

impl Default for ExaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            num_sentences: 3,
            search_type: "neural".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SerpapiConfig {
    pub api_key: String,
    pub engine: String,
    pub gl: String,
    pub hl: String,
    pub safe: String,
}

impl Default for SerpapiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            engine: "google".to_string(),
            gl: "us".to_string(),
            hl: "en".to_string(),
            safe: "off".to_string(),
        }
    }
}

/// LLM integration settings (opt-in).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
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

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: String::new(),
            model: "llama3.2".to_string(),
            timeout: 30,
            expansion_enabled: true,
            summarization_enabled: true,
            llm_rerank_enabled: false,
            max_summary_words: 0,
            input_budget_factor: 3,
        }
    }
}
