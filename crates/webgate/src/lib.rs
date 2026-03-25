//! # webgate
//!
//! Denoised web search library for AI agents.
//!
//! Fetches, cleans, and reranks web content with hard caps on context size.
//! Designed to prevent context flooding in LLM pipelines.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use webgate::Config;
//!
//! // Standalone HTML cleaning — no network, no config needed
//! let result = webgate::clean("<html><body><p>Hello</p></body></html>", 8000);
//! println!("{}", result.text);
//!
//! // Fetch and clean a page
//! # async fn example() -> Result<(), webgate::WebgateError> {
//! let config = Config::default();
//! let result = webgate::fetch("https://example.com", &config).await?;
//! println!("{}", result.text);
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod scraper;
pub mod utils;

#[cfg(feature = "backends")]
pub mod backends;

#[cfg(feature = "llm")]
pub mod llm;

pub use config::Config;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Result of cleaning raw HTML into LLM-ready plain text.
///
/// Available with `default-features = false` — no network dependencies.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CleanResult {
    pub text: String,
    pub title: String,
    pub truncated: bool,
    pub char_count: usize,
}

/// Result of fetching and cleaning a single page.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FetchResult {
    pub url: String,
    pub title: String,
    pub text: String,
    pub truncated: bool,
    pub char_count: usize,
}

/// A single source in a query result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Source {
    pub id: usize,
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub content: String,
    pub truncated: bool,
}

/// A snippet-only entry from the oversampling reserve pool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnippetEntry {
    pub id: usize,
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Statistics for a query execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Stats {
    pub fetched: usize,
    pub failed: usize,
    pub gap_filled: usize,
    pub total_chars: usize,
    pub per_page_limit: usize,
    pub num_results_per_query: usize,
}

/// Result of a full search query pipeline.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryResult {
    pub queries: Vec<String>,
    pub sources: Vec<Source>,
    pub snippet_pool: Vec<SnippetEntry>,
    pub stats: Stats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_summary_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Top-level error type
// ---------------------------------------------------------------------------

/// Top-level error type for the webgate library.
#[derive(Debug, thiserror::Error)]
pub enum WebgateError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("LLM error: {0}")]
    Llm(String),
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Clean raw HTML into LLM-ready plain text.
///
/// Standalone and synchronous — no network or config required.
/// Uses the two-stage pipeline: HTML noise removal → text sterilization.
pub fn clean(raw_html: &str, max_chars: usize) -> CleanResult {
    let (text, title, truncated) = scraper::cleaner::process_page(raw_html, "", max_chars);
    let char_count = text.len();
    CleanResult {
        text,
        title,
        truncated,
        char_count,
    }
}

/// Fetch and clean a single web page.
///
/// Applies binary extension filter before making any network request.
/// Streams the response with `max_download_mb` cap — never buffers fully.
pub async fn fetch(url: &str, config: &Config) -> Result<FetchResult, WebgateError> {
    // Binary filter runs BEFORE any network request
    if utils::url::is_binary_url(url) {
        return Err(WebgateError::Parse(format!(
            "binary file URL filtered: {}",
            url
        )));
    }

    // Domain filter
    if !utils::url::is_domain_allowed(
        url,
        &config.server.blocked_domains,
        &config.server.allowed_domains,
    ) {
        return Err(WebgateError::Parse(format!(
            "URL blocked by domain filter: {}",
            url
        )));
    }

    let max_bytes = config.server.max_download_bytes();
    let timeout = config.server.search_timeout;

    let (html_map, _timing) =
        scraper::fetcher::fetch_urls(&[url.to_string()], max_bytes, timeout).await;

    let raw = match html_map.get(url) {
        Some(h) => h.clone(),
        None => {
            return Err(WebgateError::Parse(format!("fetch failed: {}", url)));
        }
    };

    let max_chars = config.server.max_result_length;
    let (text, title, truncated) = scraper::cleaner::process_page(&raw, "", max_chars);
    let char_count = text.len();

    Ok(FetchResult {
        url: url.to_string(),
        title,
        text,
        truncated,
        char_count,
    })
}

/// Execute a full search query pipeline.
///
/// Requires the `backends` feature (enabled by default).
#[cfg(feature = "backends")]
pub async fn query(
    _queries: &[&str],
    _config: &Config,
) -> Result<QueryResult, WebgateError> {
    todo!("M3: implement query pipeline")
}
