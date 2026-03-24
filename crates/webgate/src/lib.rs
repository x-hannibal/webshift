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
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::default();
//!     let result = webgate::fetch("https://example.com", &config).await?;
//!     println!("{}", result.text);
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod scraper;
pub mod backends;
pub mod llm;
pub mod utils;

pub use config::Config;

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

/// Fetch and clean a single web page.
pub async fn fetch(
    _url: &str,
    _config: &Config,
) -> Result<FetchResult, WebgateError> {
    todo!("M1: implement fetch pipeline")
}

/// Execute a full search query pipeline.
pub async fn query(
    _queries: &[&str],
    _config: &Config,
) -> Result<QueryResult, WebgateError> {
    todo!("M3: implement query pipeline")
}

/// Top-level error type for the webgate library.
#[derive(Debug, thiserror::Error)]
pub enum WebgateError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTML parsing error: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("LLM error: {0}")]
    Llm(String),
}
