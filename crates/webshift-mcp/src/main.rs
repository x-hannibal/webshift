//! MCP server entry point: tool registration and stdio transport.
//!
//! Registers three tools: `webshift_onboarding`, `webshift_fetch`, `webshift_query`.
//!
//! Binary name: `mcp-webshift` (configured in Cargo.toml).

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use rmcp::{
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_handler, tool_router, ServiceExt,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing_subscriber::EnvFilter;
use webshift::Config;

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

/// Denoised web search MCP server — native binary, zero runtime dependencies.
#[derive(Parser, Debug)]
#[command(name = "mcp-webshift", version, about)]
struct Cli {
    /// Path to webshift.toml config file.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Override default search backend (searxng, brave, tavily, exa, serpapi, google, bing, http).
    #[arg(long)]
    default_backend: Option<String>,

    // --- SearXNG ---
    /// SearXNG instance URL (WEBSHIFT_SEARXNG_URL).
    #[arg(long)]
    searxng_url: Option<String>,

    // --- Brave Search ---
    /// Brave Search API key (WEBSHIFT_BRAVE_API_KEY).
    #[arg(long)]
    brave_api_key: Option<String>,

    // --- Tavily ---
    /// Tavily API key (WEBSHIFT_TAVILY_API_KEY).
    #[arg(long)]
    tavily_api_key: Option<String>,

    // --- Exa ---
    /// Exa API key (WEBSHIFT_EXA_API_KEY).
    #[arg(long)]
    exa_api_key: Option<String>,

    // --- SerpAPI ---
    /// SerpAPI key (WEBSHIFT_SERPAPI_API_KEY).
    #[arg(long)]
    serpapi_api_key: Option<String>,

    // --- Google Custom Search ---
    /// Google Custom Search API key (WEBSHIFT_GOOGLE_API_KEY).
    #[arg(long)]
    google_api_key: Option<String>,

    /// Google Custom Search Engine ID (WEBSHIFT_GOOGLE_CX).
    #[arg(long)]
    google_cx: Option<String>,

    // --- Bing Web Search ---
    /// Bing Web Search API key (WEBSHIFT_BING_API_KEY).
    #[arg(long)]
    bing_api_key: Option<String>,

    /// Bing market code, e.g. "en-US" (WEBSHIFT_BING_MARKET).
    #[arg(long)]
    bing_market: Option<String>,

    /// Enable debug logging.
    #[arg(long)]
    debug: bool,

    /// Enable trace-level logging.
    #[arg(long)]
    trace: bool,

    /// Log file path (logs to file instead of stderr when set).
    #[arg(long)]
    log_file: Option<String>,

    // --- Server / anti-flooding caps ---
    /// Streaming download cap per page in megabytes (WEBSHIFT_MAX_DOWNLOAD_MB).
    #[arg(long)]
    max_download_mb: Option<u32>,

    /// Hard character cap per cleaned page (WEBSHIFT_MAX_RESULT_LENGTH).
    #[arg(long)]
    max_result_length: Option<usize>,

    /// Per-request timeout in seconds for fetch and search calls (WEBSHIFT_SEARCH_TIMEOUT).
    #[arg(long)]
    search_timeout: Option<u64>,

    /// Oversample multiplier: fetch results_per_query * factor candidates (WEBSHIFT_OVERSAMPLING_FACTOR).
    #[arg(long)]
    oversampling_factor: Option<u32>,

    /// Replace failed fetches with reserve-pool pages (WEBSHIFT_AUTO_RECOVERY_FETCH).
    #[arg(long)]
    auto_recovery_fetch: Option<bool>,

    /// Hard cap on total results returned per query call (WEBSHIFT_MAX_TOTAL_RESULTS).
    #[arg(long)]
    max_total_results: Option<usize>,

    /// Total character budget across all sources in a single query (WEBSHIFT_MAX_QUERY_BUDGET).
    #[arg(long)]
    max_query_budget: Option<usize>,

    /// Maximum number of search queries per call, including LLM expansions (WEBSHIFT_MAX_SEARCH_QUERIES).
    #[arg(long)]
    max_search_queries: Option<usize>,

    /// Results requested per backend query (WEBSHIFT_RESULTS_PER_QUERY).
    #[arg(long)]
    results_per_query: Option<usize>,

    /// Comma-separated domain blocklist, e.g. "spam.com,ads.net" (WEBSHIFT_BLOCKED_DOMAINS).
    #[arg(long)]
    blocked_domains: Option<String>,

    /// Comma-separated domain allowlist — when set, only these domains pass (WEBSHIFT_ALLOWED_DOMAINS).
    #[arg(long)]
    allowed_domains: Option<String>,

    /// Proportional budget allocation after BM25 reranking: auto | on | off (WEBSHIFT_ADAPTIVE_BUDGET).
    #[arg(long)]
    adaptive_budget: Option<String>,

    /// Fetch factor for adaptive budget: max_result_length * factor before trimming (WEBSHIFT_ADAPTIVE_BUDGET_FETCH_FACTOR).
    #[arg(long)]
    adaptive_budget_fetch_factor: Option<u32>,

    /// Default language tag passed to search backends, e.g. "en", "it" (WEBSHIFT_LANGUAGE).
    #[arg(long)]
    language: Option<String>,

    // --- LLM features ---
    /// Enable LLM features (expansion, summarization, reranking).
    #[arg(long)]
    llm_enabled: Option<bool>,

    /// OpenAI-compatible API base URL (e.g. http://localhost:11434/v1).
    #[arg(long)]
    llm_base_url: Option<String>,

    /// LLM API key (leave empty for Ollama/local servers).
    #[arg(long)]
    llm_api_key: Option<String>,

    /// Model name to use for LLM calls (e.g. gemma3:27b, gpt-4o).
    #[arg(long)]
    llm_model: Option<String>,

    /// Timeout for LLM requests in seconds.
    #[arg(long)]
    llm_timeout: Option<u64>,

    /// Auto-expand single queries into complementary variants via LLM.
    #[arg(long)]
    llm_expansion_enabled: Option<bool>,

    /// Include Markdown summary with citations in query output.
    #[arg(long)]
    llm_summarization_enabled: Option<bool>,

    /// LLM-assisted reranking (deterministic BM25 always active).
    #[arg(long)]
    llm_rerank_enabled: Option<bool>,

    /// Max words in LLM summary (0 = derived from budget).
    #[arg(long)]
    llm_max_summary_words: Option<usize>,

    /// LLM input budget = max_query_budget × factor.
    #[arg(long)]
    llm_input_budget_factor: Option<u32>,
}

// ---------------------------------------------------------------------------
// Tool parameter types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchParams {
    /// The URL to retrieve.
    url: String,

    /// Character cap for returned text (default: server config value).
    /// Increase this when a previous result had truncated=true.
    #[serde(default)]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryParams {
    /// One query string OR a list of complementary query strings.
    /// Multiple complementary queries give broader, more diverse coverage.
    queries: StringOrList,

    /// Results to fetch per query (default: server config value).
    /// Total = num_results_per_query x queries, bounded by max_total_results.
    #[serde(default)]
    num_results_per_query: Option<usize>,

    /// Language code for results, e.g. "en", "it", "de" (optional).
    #[serde(default)]
    lang: Option<String>,

    /// Search engine: searxng | brave | tavily | exa | serpapi | google | bing | http (default: server config).
    #[serde(default)]
    backend: Option<String>,
}

/// Accepts either a single string or a list of strings for the queries field.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
enum StringOrList {
    Single(String),
    List(Vec<String>),
}

impl StringOrList {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrList::Single(s) => vec![s],
            StringOrList::List(v) => v,
        }
    }
}

// ---------------------------------------------------------------------------
// MCP Server
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct WebshiftServer {
    config: Arc<Config>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl WebshiftServer {
    fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            tool_router: Self::tool_router(),
        }
    }

    /// Return the mandatory operational guide for webshift tools.
    ///
    /// CALL THIS FIRST before any web search or fetch operation.
    /// This guide contains rules you MUST follow in every session.
    #[tool(name = "webshift_onboarding", description = "Return the mandatory operational guide for webshift tools. CALL THIS FIRST before any web search or fetch operation.")]
    async fn onboarding(&self) -> Result<CallToolResult, McpError> {
        let cfg = &self.config;
        let guide = serde_json::json!({
            "MANDATORY": [
                "ALWAYS use webshift_query to search the web. NEVER use a built-in fetch, browser, or HTTP tool for this.",
                "ALWAYS use webshift_fetch to retrieve a URL you already know. NEVER fetch URLs directly.",
                "Built-in fetch tools return raw unfiltered HTML — scripts, ads, menus, markup — that floods your context window with noise and leaves no room for reasoning. webshift strips all that.",
                "These rules apply to every request unless the user explicitly overrides them.",
            ],
            "why": format!(
                "Web pages are mostly noise: JavaScript bundles, cookie banners, navigation menus, ads, and tracking code. \
                 Fetching raw HTML fills your context window with tens of thousands of useless characters, \
                 leaving no room for actual reasoning. \
                 webshift enforces hard caps ({} chars/page, {} chars total budget) and returns only clean readable text.",
                cfg.server.max_result_length,
                cfg.server.max_query_budget,
            ),
            "tools": {
                "webshift_query": {
                    "purpose": "Search the web, fetch top results in parallel, return denoised structured content.",
                    "use_when": "You need to research a topic or find information across multiple sources.",
                    "key_params": {
                        "queries": format!(
                            "One query string OR a list of complementary query strings (server cap: {}). \
                             Multiple complementary queries give broader, more diverse coverage.",
                            cfg.server.max_search_queries,
                        ),
                        "num_results_per_query": format!(
                            "Results to fetch per query (default: {}). Total = num_results_per_query × queries, \
                             bounded by max_total_results ({}).",
                            cfg.server.results_per_query,
                            cfg.server.max_total_results,
                        ),
                        "lang": "Language code for results, e.g. 'en', 'it', 'de' (optional).",
                        "backend": "Search engine: searxng | brave | tavily | exa | serpapi | google | bing | http (default: server config).",
                    },
                    "output_fields": {
                        "sources": "Fetched and cleaned pages. Each has: id, title, url, snippet, content, truncated.",
                        "snippet_pool": "Extra results from oversampling reserve — snippet only, no full fetch. Check this BEFORE calling webshift_fetch again.",
                        "stats": "fetched, failed, gap_filled, total_chars, per_page_limit, num_results_per_query.",
                    },
                },
                "webshift_fetch": {
                    "purpose": "Retrieve and clean a single URL you already know.",
                    "use_when": "You have a specific URL and need its full content.",
                    "key_params": {
                        "url": "The URL to fetch.",
                        "max_chars": format!(
                            "Character cap for returned text (default: {}). Increase this if a source came back truncated=true.",
                            cfg.server.max_result_length,
                        ),
                    },
                },
            },
            "rules": [
                "Check snippet_pool BEFORE issuing more fetch calls — snippets often contain the answer.",
                "When a source has truncated=true, call webshift_fetch on that URL with a higher max_chars.",
                "Prefer multiple focused queries over one broad query — diversity beats depth for coverage.",
                "Use lang= when the user expects results in a specific language.",
            ],
            "protections": {
                "max_download_mb": format!("{} MB — hard cap on raw page download (streaming, never buffered).", cfg.server.max_download_mb),
                "max_result_length": format!("{} chars — per-page text ceiling after cleaning.", cfg.server.max_result_length),
                "max_query_budget": format!("{} chars — total char budget across all sources in one query call.", cfg.server.max_query_budget),
                "max_search_queries": format!("{} — maximum queries per call.", cfg.server.max_search_queries),
                "binary_filter": "PDF, ZIP, DOCX and other binary formats are blocked BEFORE any network request.",
                "dedup": "URLs are deduplicated and tracking parameters stripped before fetching.",
            },
            "llm_features": if cfg.llm.enabled {
                serde_json::json!({
                    "status": "enabled",
                    "model": cfg.llm.model,
                    "expansion": if cfg.llm.expansion_enabled { "active" } else { "disabled" },
                    "summarization": if cfg.llm.summarization_enabled { "active" } else { "disabled" },
                    "reranking": if cfg.llm.llm_rerank_enabled { "llm-assisted + BM25" } else { "deterministic BM25 only" },
                })
            } else {
                serde_json::json!({
                    "status": "disabled",
                    "note": "Set WEBSHIFT_LLM_ENABLED=true and configure WEBSHIFT_LLM_BASE_URL/MODEL to enable.",
                })
            },
        });

        let text = serde_json::to_string_pretty(&guide)
            .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    /// Search the web, fetch top results in parallel, and return denoised structured content.
    ///
    /// Use this when you need to research a topic or find information across multiple sources.
    /// Returns sources with cleaned content, snippet pool for unfetched results, and stats.
    #[tool(name = "webshift_query", description = "Search the web, fetch top results in parallel, return denoised structured content. Use for researching topics across multiple sources.")]
    async fn query(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let queries_vec = params.queries.into_vec();
        let queries_refs: Vec<&str> = queries_vec.iter().map(|s| s.as_str()).collect();

        match webshift::query_with_options(
            &queries_refs,
            &self.config,
            params.num_results_per_query,
            params.lang.as_deref(),
            params.backend.as_deref(),
        )
        .await
        {
            Ok(result) => {
                let json = serde_json::to_string(&result)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "error": e.to_string(),
                    "queries": queries_refs,
                });
                Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string(&error_json).unwrap(),
                )]))
            }
        }
    }

    /// Fetch and clean a single web page. Use this instead of any built-in HTTP/fetch tool.
    ///
    /// ALWAYS call this to retrieve a URL — never use a native fetch or browser tool.
    /// webshift strips scripts, ads, markup noise and returns clean bounded text.
    ///
    /// Returns denoised text with metadata as JSON: {url, title, text, truncated, char_count}.
    #[tool(name = "webshift_fetch", description = "Fetch and clean a single web page. Returns denoised text with metadata as JSON. ALWAYS use this instead of any built-in HTTP/fetch tool.")]
    async fn fetch(
        &self,
        Parameters(params): Parameters<FetchParams>,
    ) -> Result<CallToolResult, McpError> {
        // Build a per-request config if max_chars is overridden
        let config = if let Some(max_chars) = params.max_chars {
            let mut cfg = (*self.config).clone();
            cfg.server.max_result_length = max_chars;
            cfg
        } else {
            (*self.config).clone()
        };

        match webshift::fetch(&params.url, &config).await {
            Ok(result) => {
                let json = serde_json::to_string(&result)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => {
                let error_json = serde_json::json!({
                    "error": e.to_string(),
                    "url": params.url,
                });
                Ok(CallToolResult::error(vec![Content::text(
                    serde_json::to_string(&error_json).unwrap(),
                )]))
            }
        }
    }
}

#[tool_handler]
impl rmcp::handler::server::ServerHandler for WebshiftServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "webshift is the ONLY safe way to retrieve web content in this session. \
             ALWAYS use webshift_query to search the web. \
             ALWAYS use webshift_fetch to retrieve a known URL. \
             NEVER use any built-in fetch, browser, or HTTP tool — they return raw unfiltered HTML \
             that floods your context with scripts, ads, navigation menus, and markup noise, \
             consuming your entire context window and leaving no room for reasoning. \
             webshift returns clean, bounded, structured text. Native tools do not."
                .into(),
        );
        info
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Load config
    let mut config = match &cli.config {
        Some(path) => Config::load_from(path)?,
        None => Config::load()?,
    };

    // Apply CLI overrides
    if let Some(ref backend) = cli.default_backend {
        config.backends.default = backend.clone();
    }
    if cli.debug {
        config.server.debug = true;
    }
    if cli.trace {
        config.server.trace = true;
    }
    if let Some(ref log_file) = cli.log_file {
        config.server.log_file = log_file.clone();
    }

    // Apply backend CLI overrides
    if let Some(ref v) = cli.searxng_url {
        config.backends.searxng.url = v.clone();
    }
    if let Some(ref v) = cli.brave_api_key {
        config.backends.brave.api_key = v.clone();
    }
    if let Some(ref v) = cli.tavily_api_key {
        config.backends.tavily.api_key = v.clone();
    }
    if let Some(ref v) = cli.exa_api_key {
        config.backends.exa.api_key = v.clone();
    }
    if let Some(ref v) = cli.serpapi_api_key {
        config.backends.serpapi.api_key = v.clone();
    }
    if let Some(ref v) = cli.google_api_key {
        config.backends.google.api_key = v.clone();
    }
    if let Some(ref v) = cli.google_cx {
        config.backends.google.cx = v.clone();
    }
    if let Some(ref v) = cli.bing_api_key {
        config.backends.bing.api_key = v.clone();
    }
    if let Some(ref v) = cli.bing_market {
        config.backends.bing.market = v.clone();
    }

    // Apply server/anti-flooding CLI overrides
    if let Some(v) = cli.max_download_mb {
        config.server.max_download_mb = v;
    }
    if let Some(v) = cli.max_result_length {
        config.server.max_result_length = v;
    }
    if let Some(v) = cli.search_timeout {
        config.server.search_timeout = v;
    }
    if let Some(v) = cli.oversampling_factor {
        config.server.oversampling_factor = v;
    }
    if let Some(v) = cli.auto_recovery_fetch {
        config.server.auto_recovery_fetch = v;
    }
    if let Some(v) = cli.max_total_results {
        config.server.max_total_results = v;
    }
    if let Some(v) = cli.max_query_budget {
        config.server.max_query_budget = v;
    }
    if let Some(v) = cli.max_search_queries {
        config.server.max_search_queries = v;
    }
    if let Some(v) = cli.results_per_query {
        config.server.results_per_query = v;
    }
    if let Some(ref v) = cli.blocked_domains {
        config.server.blocked_domains = v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    }
    if let Some(ref v) = cli.allowed_domains {
        config.server.allowed_domains = v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    }
    if let Some(ref v) = cli.adaptive_budget {
        use webshift::config::AdaptiveBudget;
        config.server.adaptive_budget = match v.to_lowercase().as_str() {
            "on" | "true" => AdaptiveBudget::On,
            "off" | "false" => AdaptiveBudget::Off,
            _ => AdaptiveBudget::Auto,
        };
    }
    if let Some(v) = cli.adaptive_budget_fetch_factor {
        config.server.adaptive_budget_fetch_factor = v;
    }
    if let Some(ref v) = cli.language {
        config.server.language = v.clone();
    }

    // Apply LLM CLI overrides
    if let Some(v) = cli.llm_enabled {
        config.llm.enabled = v;
    }
    if let Some(ref v) = cli.llm_base_url {
        config.llm.base_url = v.clone();
    }
    if let Some(ref v) = cli.llm_api_key {
        config.llm.api_key = v.clone();
    }
    if let Some(ref v) = cli.llm_model {
        config.llm.model = v.clone();
    }
    if let Some(v) = cli.llm_timeout {
        config.llm.timeout = v;
    }
    if let Some(v) = cli.llm_expansion_enabled {
        config.llm.expansion_enabled = v;
    }
    if let Some(v) = cli.llm_summarization_enabled {
        config.llm.summarization_enabled = v;
    }
    if let Some(v) = cli.llm_rerank_enabled {
        config.llm.llm_rerank_enabled = v;
    }
    if let Some(v) = cli.llm_max_summary_words {
        config.llm.max_summary_words = v;
    }
    if let Some(v) = cli.llm_input_budget_factor {
        config.llm.input_budget_factor = v;
    }

    // Setup logging (to file or stderr)
    if config.server.debug || config.server.trace {
        let filter = if config.server.trace {
            "trace"
        } else {
            "debug"
        };

        if config.server.log_file.is_empty() {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::new(filter))
                .with_writer(std::io::stderr)
                .init();
        } else {
            let file = std::fs::File::create(&config.server.log_file)?;
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::new(filter))
                .with_writer(file)
                .with_ansi(false)
                .init();
        }

        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            backend = config.backends.default,
            budget = config.server.max_query_budget,
            max_result_length = config.server.max_result_length,
            timeout = config.server.search_timeout,
            "mcp-webshift starting",
        );
    }

    // Create server and run on stdio
    let server = WebshiftServer::new(config);
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::handler::server::ServerHandler;

    /// Extract text from the first Content item in a CallToolResult.
    fn extract_text(result: &CallToolResult) -> &str {
        result.content[0]
            .as_text()
            .expect("expected text content")
            .text
            .as_str()
    }

    #[test]
    fn server_construction() {
        let server = WebshiftServer::new(Config::default());
        assert!(server.config.server.max_result_length > 0);
    }

    #[test]
    fn server_info_has_instructions() {
        let server = WebshiftServer::new(Config::default());
        let info = server.get_info();
        let instructions = info.instructions.expect("instructions must be set");
        assert!(instructions.contains("webshift"));
        assert!(instructions.contains("NEVER"));
    }

    #[tokio::test]
    async fn onboarding_returns_valid_json() {
        let server = WebshiftServer::new(Config::default());
        let result = server.onboarding().await.expect("onboarding must succeed");
        let text = extract_text(&result);

        let guide: serde_json::Value =
            serde_json::from_str(text).expect("onboarding must return valid JSON");

        assert!(guide.get("MANDATORY").is_some(), "missing MANDATORY");
        assert!(guide.get("tools").is_some(), "missing tools");
        assert!(guide.get("rules").is_some(), "missing rules");
        assert!(guide.get("protections").is_some(), "missing protections");
        assert!(guide.get("why").is_some(), "missing why");

        let tools = guide.get("tools").unwrap();
        assert!(tools.get("webshift_query").is_some());
        assert!(tools.get("webshift_fetch").is_some());
    }

    #[tokio::test]
    async fn onboarding_reflects_config_values() {
        let mut config = Config::default();
        config.server.max_result_length = 12345;
        config.server.max_query_budget = 99999;

        let server = WebshiftServer::new(config);
        let result = server.onboarding().await.unwrap();
        let text = extract_text(&result);

        assert!(text.contains("12345"), "should reflect max_result_length");
        assert!(text.contains("99999"), "should reflect max_query_budget");
    }

    #[tokio::test]
    async fn onboarding_llm_disabled_by_default() {
        let server = WebshiftServer::new(Config::default());
        let result = server.onboarding().await.unwrap();
        let text = extract_text(&result);

        let guide: serde_json::Value = serde_json::from_str(text).unwrap();
        let llm = guide.get("llm_features").unwrap();
        assert_eq!(llm.get("status").unwrap(), "disabled");
    }

    #[tokio::test]
    async fn onboarding_llm_enabled() {
        let mut config = Config::default();
        config.llm.enabled = true;
        config.llm.model = "test-model".to_string();
        config.llm.expansion_enabled = true;
        config.llm.summarization_enabled = false;
        config.llm.llm_rerank_enabled = true;

        let server = WebshiftServer::new(config);
        let result = server.onboarding().await.unwrap();
        let text = extract_text(&result);

        let guide: serde_json::Value = serde_json::from_str(text).unwrap();
        let llm = guide.get("llm_features").unwrap();
        assert_eq!(llm.get("status").unwrap(), "enabled");
        assert_eq!(llm.get("model").unwrap(), "test-model");
        assert_eq!(llm.get("expansion").unwrap(), "active");
        assert_eq!(llm.get("summarization").unwrap(), "disabled");
        assert!(llm.get("reranking").unwrap().as_str().unwrap().contains("llm"));
    }

    #[test]
    fn cli_parse_defaults() {
        let cli = Cli::parse_from(["mcp-webshift"]);
        assert!(cli.config.is_none());
        assert!(cli.default_backend.is_none());
        assert!(!cli.debug);
        assert!(!cli.trace);
        assert!(cli.log_file.is_none());
        assert!(cli.llm_enabled.is_none());
        assert!(cli.llm_model.is_none());
        assert!(cli.llm_base_url.is_none());
    }

    #[test]
    fn cli_parse_llm_args() {
        let cli = Cli::parse_from([
            "mcp-webshift",
            "--llm-enabled",
            "true",
            "--llm-model",
            "gemma3:27b",
            "--llm-base-url",
            "http://localhost:11434/v1",
            "--llm-timeout",
            "60",
            "--llm-expansion-enabled",
            "true",
            "--llm-summarization-enabled",
            "false",
            "--llm-rerank-enabled",
            "true",
            "--llm-max-summary-words",
            "800",
        ]);
        assert_eq!(cli.llm_enabled, Some(true));
        assert_eq!(cli.llm_model.as_deref(), Some("gemma3:27b"));
        assert_eq!(
            cli.llm_base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert_eq!(cli.llm_timeout, Some(60));
        assert_eq!(cli.llm_expansion_enabled, Some(true));
        assert_eq!(cli.llm_summarization_enabled, Some(false));
        assert_eq!(cli.llm_rerank_enabled, Some(true));
        assert_eq!(cli.llm_max_summary_words, Some(800));
    }

    #[test]
    fn cli_parse_all_args() {
        let cli = Cli::parse_from([
            "mcp-webshift",
            "--config",
            "/tmp/webshift.toml",
            "--default-backend",
            "brave",
            "--searxng-url",
            "http://my-searxng:9090",
            "--brave-api-key",
            "BSA-xxx",
            "--tavily-api-key",
            "tvly-xxx",
            "--exa-api-key",
            "exa-xxx",
            "--serpapi-api-key",
            "serp-xxx",
            "--google-api-key",
            "AIza-xxx",
            "--google-cx",
            "cx-xxx",
            "--bing-api-key",
            "bing-xxx",
            "--bing-market",
            "it-IT",
            "--debug",
            "--trace",
            "--log-file",
            "/tmp/mcp.log",
        ]);
        assert_eq!(cli.config.unwrap().to_str().unwrap(), "/tmp/webshift.toml");
        assert_eq!(cli.default_backend.unwrap(), "brave");
        assert_eq!(cli.searxng_url.unwrap(), "http://my-searxng:9090");
        assert_eq!(cli.brave_api_key.unwrap(), "BSA-xxx");
        assert_eq!(cli.tavily_api_key.unwrap(), "tvly-xxx");
        assert_eq!(cli.exa_api_key.unwrap(), "exa-xxx");
        assert_eq!(cli.serpapi_api_key.unwrap(), "serp-xxx");
        assert_eq!(cli.google_api_key.unwrap(), "AIza-xxx");
        assert_eq!(cli.google_cx.unwrap(), "cx-xxx");
        assert_eq!(cli.bing_api_key.unwrap(), "bing-xxx");
        assert_eq!(cli.bing_market.unwrap(), "it-IT");
        assert!(cli.debug);
        assert!(cli.trace);
        assert_eq!(cli.log_file.unwrap(), "/tmp/mcp.log");
    }

    #[test]
    fn fetch_params_deserialize() {
        let json = r#"{"url": "https://example.com"}"#;
        let params: FetchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.url, "https://example.com");
        assert!(params.max_chars.is_none());
    }

    #[test]
    fn fetch_params_with_max_chars() {
        let json = r#"{"url": "https://example.com", "max_chars": 16000}"#;
        let params: FetchParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.url, "https://example.com");
        assert_eq!(params.max_chars.unwrap(), 16000);
    }

    #[test]
    fn query_params_single_string() {
        let json = r#"{"queries": "rust async"}"#;
        let params: QueryParams = serde_json::from_str(json).unwrap();
        let queries = params.queries.into_vec();
        assert_eq!(queries, vec!["rust async"]);
        assert!(params.num_results_per_query.is_none());
        assert!(params.lang.is_none());
        assert!(params.backend.is_none());
    }

    #[test]
    fn query_params_list_of_strings() {
        let json = r#"{"queries": ["rust async", "tokio tutorial"], "num_results_per_query": 3, "lang": "en"}"#;
        let params: QueryParams = serde_json::from_str(json).unwrap();
        let queries = params.queries.into_vec();
        assert_eq!(queries, vec!["rust async", "tokio tutorial"]);
        assert_eq!(params.num_results_per_query.unwrap(), 3);
        assert_eq!(params.lang.unwrap(), "en");
    }

    #[test]
    fn string_or_list_single_into_vec() {
        let s = StringOrList::Single("x".into());
        assert_eq!(s.into_vec(), vec!["x"]);
    }

    #[test]
    fn string_or_list_list_into_vec() {
        let s = StringOrList::List(vec!["a".into(), "b".into()]);
        assert_eq!(s.into_vec(), vec!["a", "b"]);
    }

    #[test]
    fn query_params_empty_list_deserialize() {
        let json = r#"{"queries": []}"#;
        let params: QueryParams = serde_json::from_str(json).unwrap();
        let queries = params.queries.into_vec();
        assert!(queries.is_empty());
    }

    #[test]
    fn query_params_with_backend_override() {
        let json = r#"{"queries": "test query", "backend": "brave"}"#;
        let params: QueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.backend.unwrap(), "brave");
    }
}
