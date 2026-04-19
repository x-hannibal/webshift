//! Configuration system: CLI args > env vars > webshift.toml > defaults.
//!
//! The library handles TOML loading and env var overrides.
//! CLI parsing belongs in the binary crate (`webshift-mcp`).

use serde::{de, Deserialize};
use std::fmt;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// AdaptiveBudget mode
// ---------------------------------------------------------------------------

/// Controls proportional budget allocation after BM25 reranking.
///
/// - `Off` — flat per-page cap (`max_query_budget / num_sources`), always.
/// - `On`  — always redistribute budget proportionally to BM25 scores.
/// - `Auto` (default) — redistribute only when scores are sufficiently spread
///   (dominance ratio > 1.5, i.e. the top source would receive 50%+ more than flat).
///
/// TOML: `adaptive_budget = "auto"` / `"on"` / `"off"` / `true` / `false`
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AdaptiveBudget {
    /// Redistribute only when BM25 score spread exceeds the dominance threshold.
    #[default]
    Auto,
    /// Always redistribute budget proportionally to BM25 scores.
    On,
    /// Flat per-page cap, no proportional redistribution.
    Off,
}

impl fmt::Display for AdaptiveBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdaptiveBudget::Auto => write!(f, "auto"),
            AdaptiveBudget::On => write!(f, "on"),
            AdaptiveBudget::Off => write!(f, "off"),
        }
    }
}

impl<'de> Deserialize<'de> for AdaptiveBudget {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = AdaptiveBudget;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, r#"bool or "auto" / "on" / "off""#)
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<AdaptiveBudget, E> {
                Ok(if v { AdaptiveBudget::On } else { AdaptiveBudget::Off })
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<AdaptiveBudget, E> {
                match v.to_lowercase().as_str() {
                    "auto" => Ok(AdaptiveBudget::Auto),
                    "on" | "true" | "yes" | "1" => Ok(AdaptiveBudget::On),
                    "off" | "false" | "no" | "0" => Ok(AdaptiveBudget::Off),
                    _ => Err(de::Error::invalid_value(de::Unexpected::Str(v), &self)),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// Top-level configuration.
///
/// Loaded via [`Config::load()`] (searches for `webshift.toml` then applies
/// `WEBSHIFT_*` env overrides) or constructed directly for embedding.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Server-level settings and anti-flooding caps.
    pub server: ServerConfig,
    /// Search backend selection and per-backend credentials.
    pub backends: BackendsConfig,
    /// Optional LLM integration settings (requires `llm` feature).
    pub llm: LlmConfig,
}

/// Server-level settings and anti-flooding caps.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Streaming download cap per page in megabytes (default: 1).
    pub max_download_mb: u32,
    /// Hard character cap per cleaned page (default: 8000). Set to `0` to
    /// disable the cap entirely (no truncation).
    pub max_result_length: usize,
    /// Per-request timeout in seconds for fetch and search calls (default: 8).
    pub search_timeout: u64,
    /// Oversample multiplier: fetch `results_per_query * factor` candidates (default: 2).
    pub oversampling_factor: u32,
    /// When `true`, replace failed fetches with reserve-pool pages (default: false).
    pub auto_recovery_fetch: bool,
    /// Hard cap on total results returned per query call (default: 20).
    pub max_total_results: usize,
    /// Total character budget across all sources in a single query (default: 32000).
    pub max_query_budget: usize,
    /// Maximum number of search queries per call, including LLM expansions (default: 5).
    pub max_search_queries: usize,
    /// Results requested per backend query (default: 5).
    pub results_per_query: usize,
    /// Domain blocklist — URLs matching these domains are silently dropped.
    pub blocked_domains: Vec<String>,
    /// Domain allowlist — when non-empty, only these domains pass the filter.
    pub allowed_domains: Vec<String>,
    /// Enable debug-level logging (default: false).
    pub debug: bool,
    /// Path to a log file (empty = stderr only).
    pub log_file: String,
    /// Enable trace-level logging (default: false).
    pub trace: bool,
    /// Controls proportional budget allocation after BM25 reranking.
    pub adaptive_budget: AdaptiveBudget,
    /// Fetch factor for adaptive budget: fetch up to `max_result_length * factor` chars
    /// before trimming proportionally (default: 3).
    pub adaptive_budget_fetch_factor: u32,
    /// BCP-47 language tag passed to search backends (e.g. "en", "it", "all").
    /// Empty string = let the backend decide.
    pub language: String,
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
            adaptive_budget: AdaptiveBudget::Auto,
            adaptive_budget_fetch_factor: 3,
            language: "en".to_string(),
        }
    }
}

impl ServerConfig {
    /// Hard cap in bytes for streaming download.
    pub fn max_download_bytes(&self) -> usize {
        self.max_download_mb as usize * 1024 * 1024
    }
}

/// Backend selection and per-backend credentials/settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BackendsConfig {
    /// Name of the default backend (e.g. `"searxng"`, `"brave"`, `"google"`).
    pub default: String,
    pub searxng: SearxngConfig,
    pub brave: BraveConfig,
    pub tavily: TavilyConfig,
    pub exa: ExaConfig,
    pub serpapi: SerpapiConfig,
    pub google: GoogleConfig,
    pub bing: BingConfig,
    pub http: HttpBackendConfig,
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
            google: GoogleConfig::default(),
            bing: BingConfig::default(),
            http: HttpBackendConfig::default(),
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

/// Google Custom Search API config.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GoogleConfig {
    pub api_key: String,
    /// Custom Search Engine ID (from programmablesearchengine.google.com).
    pub cx: String,
}

/// Bing Web Search API config.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BingConfig {
    pub api_key: String,
    /// Market code, e.g. "en-US", "it-IT" (default: "en-US").
    pub market: String,
}

impl Default for BingConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            market: "en-US".to_string(),
        }
    }
}

/// Generic configurable HTTP backend.
///
/// Point at any REST search API that returns a JSON array of results.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HttpBackendConfig {
    /// Base URL of the search endpoint.
    pub url: String,
    /// HTTP method: "GET" (default) or "POST".
    pub method: String,
    /// Query parameter name for the search string (default: "q").
    pub query_param: String,
    /// Query parameter name for the result count (default: "count"; set to ""
    /// to omit).
    pub count_param: String,
    /// Query parameter name for the language filter (default: ""; set to ""
    /// to omit).
    pub lang_param: String,
    /// Dot-separated JSON path to the results array, e.g. "data.items".
    /// Empty string means the root of the response is the array.
    pub results_path: String,
    /// Field name for the result title (default: "title").
    pub title_field: String,
    /// Field name for the result URL (default: "url").
    pub url_field: String,
    /// Field name for the result snippet (default: "snippet").
    pub snippet_field: String,
    /// Additional static HTTP headers (e.g. Authorization).
    pub headers: std::collections::HashMap<String, String>,
    /// Additional static query parameters appended to every request.
    pub extra_params: std::collections::HashMap<String, String>,
}

impl Default for HttpBackendConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: "GET".to_string(),
            query_param: "q".to_string(),
            count_param: "count".to_string(),
            lang_param: String::new(),
            results_path: String::new(),
            title_field: "title".to_string(),
            url_field: "url".to_string(),
            snippet_field: "snippet".to_string(),
            headers: std::collections::HashMap::new(),
            extra_params: std::collections::HashMap::new(),
        }
    }
}

/// LLM integration settings (opt-in, requires the `llm` feature).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// Master switch for all LLM features (default: false).
    pub enabled: bool,
    /// Base URL of an OpenAI-compatible API (default: local Ollama).
    pub base_url: String,
    /// API key for the LLM endpoint.
    pub api_key: String,
    /// Model name to use (default: `"llama3.2"`).
    pub model: String,
    /// Request timeout in seconds (default: 30).
    pub timeout: u64,
    /// Enable LLM-based query expansion (default: true when `enabled`).
    pub expansion_enabled: bool,
    /// Enable LLM-based summarization of results (default: true when `enabled`).
    pub summarization_enabled: bool,
    /// Enable LLM-assisted reranking as a second pass after BM25 (default: false).
    pub llm_rerank_enabled: bool,
    /// Maximum word count for the summary (0 = auto from budget).
    pub max_summary_words: usize,
    /// Multiplier for the input budget sent to the summarizer (default: 3).
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

// ---------------------------------------------------------------------------
// Loading: TOML file → env overrides
// ---------------------------------------------------------------------------

impl Config {
    /// Load config: find `webshift.toml` (CWD then home), apply env overrides.
    pub fn load() -> Result<Self, crate::WebshiftError> {
        let mut cfg = match find_config_file() {
            Some(path) => Self::load_from(&path)?,
            None => Self::default(),
        };
        cfg.apply_env();
        Ok(cfg)
    }

    /// Load config from a specific TOML file, then apply env overrides.
    pub fn load_from(path: &Path) -> Result<Self, crate::WebshiftError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::WebshiftError::Config(format!("cannot read {}: {e}", path.display()))
        })?;
        let mut cfg: Config = toml::from_str(&content).map_err(|e| {
            crate::WebshiftError::Config(format!("invalid TOML in {}: {e}", path.display()))
        })?;
        cfg.apply_env();
        Ok(cfg)
    }

    /// Override fields from `WEBSHIFT_*` environment variables.
    fn apply_env(&mut self) {
        use std::env;

        fn env_str(key: &str) -> Option<String> {
            env::var(key).ok()
        }

        fn env_bool(key: &str) -> Option<bool> {
            env::var(key)
                .ok()
                .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        }

        fn env_u32(key: &str) -> Option<u32> {
            env::var(key).ok().and_then(|v| v.parse().ok())
        }

        fn env_u64(key: &str) -> Option<u64> {
            env::var(key).ok().and_then(|v| v.parse().ok())
        }

        fn env_usize(key: &str) -> Option<usize> {
            env::var(key).ok().and_then(|v| v.parse().ok())
        }

        // server
        if let Some(v) = env_u32("WEBSHIFT_MAX_DOWNLOAD_MB") {
            self.server.max_download_mb = v;
        }
        if let Some(v) = env_usize("WEBSHIFT_MAX_RESULT_LENGTH") {
            self.server.max_result_length = v;
        }
        if let Some(v) = env_u64("WEBSHIFT_SEARCH_TIMEOUT") {
            self.server.search_timeout = v;
        }
        if let Some(v) = env_u32("WEBSHIFT_OVERSAMPLING_FACTOR") {
            self.server.oversampling_factor = v;
        }
        if let Some(v) = env_bool("WEBSHIFT_AUTO_RECOVERY_FETCH") {
            self.server.auto_recovery_fetch = v;
        }
        if let Some(v) = env_usize("WEBSHIFT_MAX_TOTAL_RESULTS") {
            self.server.max_total_results = v;
        }
        if let Some(v) = env_usize("WEBSHIFT_MAX_QUERY_BUDGET") {
            self.server.max_query_budget = v;
        }
        if let Some(v) = env_usize("WEBSHIFT_MAX_SEARCH_QUERIES") {
            self.server.max_search_queries = v;
        }
        if let Some(v) = env_usize("WEBSHIFT_RESULTS_PER_QUERY") {
            self.server.results_per_query = v;
        }
        if let Some(v) = env_bool("WEBSHIFT_DEBUG") {
            self.server.debug = v;
        }
        if let Some(v) = env_str("WEBSHIFT_LOG_FILE") {
            self.server.log_file = v;
        }
        if let Some(v) = env_bool("WEBSHIFT_TRACE") {
            self.server.trace = v;
        }
        if let Some(v) = env_str("WEBSHIFT_ADAPTIVE_BUDGET") {
            self.server.adaptive_budget = match v.to_lowercase().as_str() {
                "auto" => AdaptiveBudget::Auto,
                "on" | "true" | "yes" | "1" => AdaptiveBudget::On,
                _ => AdaptiveBudget::Off,
            };
        }
        if let Some(v) = env_u32("WEBSHIFT_ADAPTIVE_BUDGET_FETCH_FACTOR") {
            self.server.adaptive_budget_fetch_factor = v;
        }

        // backends
        if let Some(v) = env_str("WEBSHIFT_DEFAULT_BACKEND") {
            self.backends.default = v;
        }
        if let Some(v) = env_str("WEBSHIFT_SEARXNG_URL") {
            self.backends.searxng.url = v;
        }
        if let Some(v) = env_str("WEBSHIFT_BRAVE_API_KEY") {
            self.backends.brave.api_key = v;
        }
        if let Some(v) = env_str("WEBSHIFT_TAVILY_API_KEY") {
            self.backends.tavily.api_key = v;
        }
        if let Some(v) = env_str("WEBSHIFT_EXA_API_KEY") {
            self.backends.exa.api_key = v;
        }
        if let Some(v) = env_str("WEBSHIFT_SERPAPI_API_KEY") {
            self.backends.serpapi.api_key = v;
        }
        if let Some(v) = env_str("WEBSHIFT_SERPAPI_ENGINE") {
            self.backends.serpapi.engine = v;
        }
        if let Some(v) = env_str("WEBSHIFT_SERPAPI_GL") {
            self.backends.serpapi.gl = v;
        }
        if let Some(v) = env_str("WEBSHIFT_SERPAPI_HL") {
            self.backends.serpapi.hl = v;
        }

        // llm
        if let Some(v) = env_bool("WEBSHIFT_LLM_ENABLED") {
            self.llm.enabled = v;
        }
        if let Some(v) = env_str("WEBSHIFT_LLM_BASE_URL") {
            self.llm.base_url = v;
        }
        if let Some(v) = env_str("WEBSHIFT_LLM_API_KEY") {
            self.llm.api_key = v;
        }
        if let Some(v) = env_str("WEBSHIFT_LLM_MODEL") {
            self.llm.model = v;
        }
        if let Some(v) = env_u64("WEBSHIFT_LLM_TIMEOUT") {
            self.llm.timeout = v;
        }
        if let Some(v) = env_bool("WEBSHIFT_LLM_EXPANSION_ENABLED") {
            self.llm.expansion_enabled = v;
        }
        if let Some(v) = env_bool("WEBSHIFT_LLM_SUMMARIZATION_ENABLED") {
            self.llm.summarization_enabled = v;
        }
        if let Some(v) = env_bool("WEBSHIFT_LLM_RERANK_ENABLED") {
            self.llm.llm_rerank_enabled = v;
        }
        if let Some(v) = env_usize("WEBSHIFT_LLM_MAX_SUMMARY_WORDS") {
            self.llm.max_summary_words = v;
        }
        if let Some(v) = env_u32("WEBSHIFT_LLM_INPUT_BUDGET_FACTOR") {
            self.llm.input_budget_factor = v;
        }
    }
}

/// Search for `webshift.toml` in CWD then home directory.
fn find_config_file() -> Option<PathBuf> {
    let candidates = [
        std::env::current_dir().ok().map(|d| d.join("webshift.toml")),
        dirs_next::home_dir().map(|d| d.join("webshift.toml")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_python() {
        let cfg = Config::default();
        assert_eq!(cfg.server.max_download_mb, 1);
        assert_eq!(cfg.server.max_result_length, 8000);
        assert_eq!(cfg.server.search_timeout, 8);
        assert_eq!(cfg.server.oversampling_factor, 2);
        assert!(!cfg.server.auto_recovery_fetch);
        assert_eq!(cfg.server.max_total_results, 20);
        assert_eq!(cfg.server.max_query_budget, 32000);
        assert_eq!(cfg.server.max_search_queries, 5);
        assert_eq!(cfg.server.results_per_query, 5);
        assert!(!cfg.server.debug);
        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::Auto);
        assert_eq!(cfg.server.adaptive_budget_fetch_factor, 3);
        assert_eq!(cfg.backends.default, "searxng");
        assert_eq!(cfg.backends.searxng.url, "http://localhost:8080");
        assert!(!cfg.llm.enabled);
        assert_eq!(cfg.llm.model, "llama3.2");
        assert_eq!(cfg.llm.timeout, 30);
        assert_eq!(cfg.llm.input_budget_factor, 3);
    }

    #[test]
    fn max_download_bytes_conversion() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.max_download_bytes(), 1024 * 1024);
    }

    #[test]
    fn toml_partial_override() {
        let toml_str = r#"
[server]
max_download_mb = 5
max_result_length = 16000

[backends.searxng]
url = "http://my-searxng:9090"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.server.max_download_mb, 5);
        assert_eq!(cfg.server.max_result_length, 16000);
        // Non-specified fields keep defaults
        assert_eq!(cfg.server.search_timeout, 8);
        assert_eq!(cfg.backends.searxng.url, "http://my-searxng:9090");
        assert_eq!(cfg.backends.default, "searxng");
    }

    #[test]
    fn env_override() {
        // Safety: test runs single-threaded; env vars are cleaned up after.
        unsafe {
            std::env::set_var("WEBSHIFT_MAX_DOWNLOAD_MB", "10");
            std::env::set_var("WEBSHIFT_DEBUG", "true");
            std::env::set_var("WEBSHIFT_SEARXNG_URL", "http://test:1234");
            std::env::set_var("WEBSHIFT_LLM_MODEL", "gpt-4o");
        }

        let mut cfg = Config::default();
        cfg.apply_env();

        assert_eq!(cfg.server.max_download_mb, 10);
        assert!(cfg.server.debug);
        assert_eq!(cfg.backends.searxng.url, "http://test:1234");
        assert_eq!(cfg.llm.model, "gpt-4o");

        unsafe {
            std::env::remove_var("WEBSHIFT_MAX_DOWNLOAD_MB");
            std::env::remove_var("WEBSHIFT_DEBUG");
            std::env::remove_var("WEBSHIFT_SEARXNG_URL");
            std::env::remove_var("WEBSHIFT_LLM_MODEL");
        }
    }

    // --- AdaptiveBudget deserialization ---

    #[test]
    fn adaptive_budget_deserialize_bool_true() {
        let toml_str = r#"
[server]
adaptive_budget = true
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::On);
    }

    #[test]
    fn adaptive_budget_deserialize_bool_false() {
        let toml_str = r#"
[server]
adaptive_budget = false
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::Off);
    }

    #[test]
    fn adaptive_budget_deserialize_string_auto() {
        let toml_str = r#"
[server]
adaptive_budget = "auto"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::Auto);
    }

    #[test]
    fn adaptive_budget_deserialize_string_on() {
        let toml_str = r#"
[server]
adaptive_budget = "on"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::On);
    }

    #[test]
    fn adaptive_budget_deserialize_string_off() {
        let toml_str = r#"
[server]
adaptive_budget = "off"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::Off);
    }

    #[test]
    fn adaptive_budget_display() {
        assert_eq!(AdaptiveBudget::Auto.to_string(), "auto");
        assert_eq!(AdaptiveBudget::On.to_string(), "on");
        assert_eq!(AdaptiveBudget::Off.to_string(), "off");
    }

    // --- Env var override tests ---

    #[test]
    fn env_override_adaptive_budget() {
        unsafe {
            std::env::set_var("WEBSHIFT_ADAPTIVE_BUDGET", "auto");
        }

        let mut cfg = Config::default();
        // Start with a non-Auto value to prove the override works
        cfg.server.adaptive_budget = AdaptiveBudget::Off;
        cfg.apply_env();

        assert_eq!(cfg.server.adaptive_budget, AdaptiveBudget::Auto);

        unsafe {
            std::env::remove_var("WEBSHIFT_ADAPTIVE_BUDGET");
        }
    }

    #[test]
    fn env_override_llm_enabled() {
        unsafe {
            std::env::set_var("WEBSHIFT_LLM_ENABLED", "true");
        }

        let mut cfg = Config::default();
        assert!(!cfg.llm.enabled, "default should be false");
        cfg.apply_env();

        assert!(cfg.llm.enabled, "env override should set llm.enabled to true");

        unsafe {
            std::env::remove_var("WEBSHIFT_LLM_ENABLED");
        }
    }

    #[test]
    fn env_bool_accepts_yes_and_1() {
        // Test "yes"
        unsafe {
            std::env::set_var("WEBSHIFT_DEBUG", "yes");
        }
        let mut cfg = Config::default();
        cfg.apply_env();
        assert!(cfg.server.debug, "env_bool should accept 'yes'");

        // Test "1"
        unsafe {
            std::env::set_var("WEBSHIFT_DEBUG", "1");
        }
        let mut cfg = Config::default();
        cfg.apply_env();
        assert!(cfg.server.debug, "env_bool should accept '1'");

        // Test "true"
        unsafe {
            std::env::set_var("WEBSHIFT_DEBUG", "true");
        }
        let mut cfg = Config::default();
        cfg.apply_env();
        assert!(cfg.server.debug, "env_bool should accept 'true'");

        unsafe {
            std::env::remove_var("WEBSHIFT_DEBUG");
        }
    }

    #[test]
    fn default_language_is_en() {
        assert_eq!(ServerConfig::default().language, "en");
    }
}
