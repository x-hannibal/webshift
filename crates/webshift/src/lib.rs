#![cfg_attr(docsrs, feature(doc_cfg))]

//! # webshift
//!
//! Denoised web search library for AI agents. Webshift fetches, cleans, reranks,
//! and budget-caps web content so that LLM pipelines receive high-signal context
//! without flooding their context windows. Every code path enforces hard limits
//! on download size, per-page character count, and total query budget.
//!
//! ## Feature flags
//!
//! | Feature | Default | Enables |
//! |---------|---------|---------|
//! | `backends` | **on** | All 8 search backends (SearXNG, Brave, Tavily, Exa, SerpAPI, Google, Bing, HTTP) and the [`query()`] pipeline |
//! | `llm` | off | OpenAI-compatible LLM client, query expansion, summarization, and LLM-assisted reranking |
//!
//! Minimal dependency (cleaner + fetcher only):
//! ```toml
//! webshift = { version = "0.1", default-features = false }
//! ```
//!
//! ## Use cases
//!
//! ### HTML cleaning only (`default-features = false`)
//!
//! Synchronous, zero-network, zero-config HTML-to-text conversion:
//!
//! ```rust
//! let result = webshift::clean("<html><body><nav>menu</nav><p>Hello world</p></body></html>", 8000);
//! assert!(result.text.contains("Hello world"));
//! assert!(!result.text.contains("menu")); // noise removed
//! ```
//!
//! ### Fetch and clean a single page
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), webshift::WebshiftError> {
//! let config = webshift::Config::default();
//! let result = webshift::fetch("https://example.com", &config).await?;
//! println!("title: {}", result.title);
//! println!("text:  {}...", &result.text[..100]);
//! # Ok(())
//! # }
//! ```
//!
//! ### Full search pipeline (requires `backends`)
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), webshift::WebshiftError> {
//! let config = webshift::Config::load()?;
//! let result = webshift::query(&["rust async runtime"], &config).await?;
//! for source in &result.sources {
//!     println!("[{}] {} — {}", source.id, source.title, source.url);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Anti-flooding protections
//!
//! - `max_download_mb`: streaming cap per page (never buffers the full response)
//! - `max_result_length`: hard character cap per cleaned page
//! - `max_query_budget`: total character budget across all sources
//! - `max_total_results`: hard cap on results per call
//! - Binary extension filter runs **before** any network request
//! - Unicode/BiDi sterilization in the cleaner

pub mod config;
pub mod scraper;
pub mod utils;

#[cfg(feature = "backends")]
#[cfg_attr(docsrs, doc(cfg(feature = "backends")))]
pub mod backends;

#[cfg(feature = "llm")]
#[cfg_attr(docsrs, doc(cfg(feature = "llm")))]
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
    /// Cleaned plain text with HTML noise and Unicode control characters removed.
    pub text: String,
    /// Page title extracted from `<title>` or first `<h1>`, empty if not found.
    pub title: String,
    /// `true` if the output was truncated to `max_chars`.
    pub truncated: bool,
    /// Length of `text` in bytes (ASCII-safe after sterilization).
    pub char_count: usize,
}

/// Result of fetching and cleaning a single page.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FetchResult {
    /// The URL that was fetched (after any redirects, this is the original request URL).
    pub url: String,
    /// Page title extracted from `<title>` or first `<h1>`.
    pub title: String,
    /// Cleaned plain text content.
    pub text: String,
    /// `true` if the output was truncated to [`ServerConfig::max_result_length`](config::ServerConfig::max_result_length).
    pub truncated: bool,
    /// Length of `text` in bytes.
    pub char_count: usize,
}

/// A single source in a query result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Source {
    /// 1-based rank after BM25 reranking.
    pub id: usize,
    /// Page title (from HTML or backend metadata).
    pub title: String,
    /// Source URL.
    pub url: String,
    /// Backend-provided snippet, if different from `content`.
    pub snippet: Option<String>,
    /// Cleaned page content, budget-capped.
    pub content: String,
    /// `true` if `content` was truncated to fit the budget.
    pub truncated: bool,
}

/// A snippet-only entry from the oversampling reserve pool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnippetEntry {
    /// 1-based ID continuing after the last [`Source`].
    pub id: usize,
    /// Page title from backend metadata.
    pub title: String,
    /// Source URL.
    pub url: String,
    /// Backend-provided snippet text.
    pub snippet: String,
}

/// Statistics for a query execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Stats {
    /// Number of pages successfully fetched and cleaned.
    pub fetched: usize,
    /// Number of pages that failed to fetch.
    pub failed: usize,
    /// Number of reserve-pool pages used to replace failed fetches.
    pub gap_filled: usize,
    /// Total characters across all sources after budget allocation.
    pub total_chars: usize,
    /// Per-page character limit applied during this query.
    pub per_page_limit: usize,
    /// Effective results-per-query count used.
    pub num_results_per_query: usize,
    /// Total raw HTML bytes downloaded before cleaning.
    pub raw_bytes: usize,
}

/// Result of a full search query pipeline.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryResult {
    /// Search queries actually executed (may include LLM-expanded queries).
    pub queries: Vec<String>,
    /// Fetched, cleaned, and reranked sources.
    pub sources: Vec<Source>,
    /// Oversampled pages that were not fetched, available as snippet-only references.
    pub snippet_pool: Vec<SnippetEntry>,
    /// Pipeline execution statistics.
    pub stats: Stats,
    /// LLM-generated summary with citations (requires `llm` feature).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Error message if LLM summarization was attempted but failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_summary_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Top-level error type
// ---------------------------------------------------------------------------

/// Top-level error type for the webshift library.
#[derive(Debug, thiserror::Error)]
pub enum WebshiftError {
    /// Network-level error from `reqwest` (timeouts, DNS, TLS, etc.).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// HTML parsing or content extraction failure.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Invalid or missing configuration (TOML parse error, missing required field).
    #[error("Configuration error: {0}")]
    Config(String),

    /// Search backend error (unknown backend name, missing API key, API failure).
    #[error("Backend error: {0}")]
    Backend(String),

    /// LLM client error (connection failure, malformed response).
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
pub async fn fetch(url: &str, config: &Config) -> Result<FetchResult, WebshiftError> {
    // Binary filter runs BEFORE any network request
    if utils::url::is_binary_url(url) {
        return Err(WebshiftError::Parse(format!(
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
        return Err(WebshiftError::Parse(format!(
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
            return Err(WebshiftError::Parse(format!("fetch failed: {}", url)));
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
/// Searches the web via the configured backend, fetches top results in parallel,
/// cleans them, reranks by BM25, and returns structured content with snippet pool.
///
/// Requires the `backends` feature (enabled by default).
#[cfg(feature = "backends")]
#[cfg_attr(docsrs, doc(cfg(feature = "backends")))]
pub async fn query(queries: &[&str], config: &Config) -> Result<QueryResult, WebshiftError> {
    query_with_options(queries, config, None, None, None).await
}

/// Full query pipeline with optional overrides.
///
/// - `num_results_per_query`: results per query (default: config value)
/// - `lang`: language filter
/// - `backend_name`: override the default backend
#[cfg(feature = "backends")]
#[cfg_attr(docsrs, doc(cfg(feature = "backends")))]
pub async fn query_with_options(
    queries: &[&str],
    config: &Config,
    num_results_per_query: Option<usize>,
    lang: Option<&str>,
    backend_name: Option<&str>,
) -> Result<QueryResult, WebshiftError> {
    use backends::{create_backend, create_backend_by_name, SearchResult as BackendResult};

    let cfg = &config.server;

    // Create backend
    let backend = match backend_name {
        Some(name) => create_backend_by_name(name, &config.backends)?,
        None => create_backend(&config.backends)?,
    };

    // Normalize queries, enforce server cap
    let base_queries: Vec<String> = queries
        .iter()
        .take(cfg.max_search_queries)
        .map(|s| s.to_string())
        .collect();

    // LLM query expansion (single input query → multiple complementary queries)
    #[cfg(feature = "llm")]
    let queries_list: Vec<String> = if config.llm.enabled
        && config.llm.expansion_enabled
        && base_queries.len() == 1
    {
        let llm_client = llm::client::LlmClient::new(&config.llm);
        let expanded = llm::expander::expand_queries(
            &base_queries[0],
            cfg.max_search_queries,
            &llm_client,
        )
        .await;
        expanded.into_iter().take(cfg.max_search_queries).collect()
    } else {
        base_queries
    };

    #[cfg(not(feature = "llm"))]
    let queries_list: Vec<String> = base_queries;

    if queries_list.is_empty() {
        return Err(WebshiftError::Backend("no queries provided".into()));
    }

    let nrpq = num_results_per_query
        .unwrap_or(cfg.results_per_query)
        .min(cfg.max_total_results);

    // Total candidates = per-query x number of queries, hard-capped
    let total_results = (nrpq * queries_list.len()).min(cfg.max_total_results);

    // Oversample per query for signal density after cross-query dedup
    let oversample_count = nrpq * cfg.oversampling_factor as usize;

    // Resolve language: explicit param wins, then config, then none
    let resolved_lang: Option<&str> = lang.or_else(|| {
        let l = cfg.language.as_str();
        if l.is_empty() { None } else { Some(l) }
    });

    // Parallel search across all queries
    let search_futures: Vec<_> = queries_list
        .iter()
        .map(|q| backend.search(q, oversample_count, resolved_lang))
        .collect();

    let results_per_query = futures::future::join_all(search_futures).await;

    // Flatten in round-robin order so no single query dominates
    let mut result_lists: Vec<Vec<BackendResult>> = Vec::new();
    for r in results_per_query {
        match r {
            Ok(list) => result_lists.push(list),
            Err(e) => {
                tracing::warn!("backend search error: {e}");
            }
        }
    }

    let max_len = result_lists.iter().map(|l| l.len()).max().unwrap_or(0);
    let mut raw_results: Vec<BackendResult> = Vec::new();
    for i in 0..max_len {
        for list in &result_lists {
            if i < list.len() {
                raw_results.push(list[i].clone());
            }
        }
    }

    // Filter and dedup
    let mut valid: Vec<BackendResult> = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    for r in &raw_results {
        let clean = utils::url::sanitize_url(&r.url).to_lowercase();
        let clean = clean.trim_end_matches('/').to_string();
        if seen_urls.contains(&clean) || utils::url::is_binary_url(&r.url) {
            continue;
        }
        if !utils::url::is_domain_allowed(&r.url, &cfg.blocked_domains, &cfg.allowed_domains) {
            continue;
        }
        seen_urls.insert(clean);
        valid.push(r.clone());
    }

    // Split into candidates (Round 1) and reserve pool
    let candidates: Vec<BackendResult> = valid.iter().take(total_results).cloned().collect();
    let mut reserve_pool: Vec<BackendResult> = valid.iter().skip(total_results).cloned().collect();

    // Round 1: parallel fetch
    let candidate_urls: Vec<String> = candidates.iter().map(|r| r.url.clone()).collect();
    let max_bytes = cfg.max_download_bytes();
    let (mut html_map, mut fetch_timing) =
        scraper::fetcher::fetch_urls(&candidate_urls, max_bytes, cfg.search_timeout).await;

    // Round 2: gap filler (replace failed fetches from reserve pool)
    let mut gap_filled: usize = 0;
    let mut final_candidates = candidates.clone();

    if cfg.auto_recovery_fetch && !reserve_pool.is_empty() {
        let failed: Vec<&BackendResult> = candidates
            .iter()
            .filter(|r| !html_map.contains_key(&r.url))
            .collect();

        if !failed.is_empty() {
            let gap_size = failed.len().min(reserve_pool.len());
            let backups: Vec<BackendResult> = reserve_pool.drain(..gap_size).collect();

            let backup_urls: Vec<String> = backups.iter().map(|r| r.url.clone()).collect();
            let (backup_html, backup_timing) =
                scraper::fetcher::fetch_urls(&backup_urls, max_bytes, cfg.search_timeout).await;

            html_map.extend(backup_html);
            fetch_timing.extend(backup_timing);

            // Rebuild candidates: keep successful, add backups
            let mut new_candidates: Vec<BackendResult> = final_candidates
                .iter()
                .filter(|r| html_map.contains_key(&r.url))
                .cloned()
                .collect();
            new_candidates.extend(backups);

            // Demote truly failed to reserve
            let still_failed: Vec<BackendResult> = final_candidates
                .iter()
                .filter(|r| !html_map.contains_key(&r.url))
                .cloned()
                .collect();
            reserve_pool = still_failed.into_iter().chain(reserve_pool).collect();

            gap_filled = gap_size;
            final_candidates = new_candidates;
        }
    }

    // Per-page char limit
    let per_page_limit = cfg
        .max_result_length
        .min(cfg.max_query_budget / final_candidates.len().max(1));

    let fetch_limit = if cfg.adaptive_budget != config::AdaptiveBudget::Off {
        cfg.max_result_length * cfg.adaptive_budget_fetch_factor as usize
    } else {
        per_page_limit
    };

    // Process fetched pages
    let mut sources: Vec<Source> = Vec::new();
    let mut fetched_count: usize = 0;
    let mut failed_count: usize = 0;

    for (idx, result) in final_candidates.iter().enumerate() {
        let raw = html_map.get(&result.url);
        if let Some(raw) = raw {
            let (text, title, truncated) =
                scraper::cleaner::process_page(raw, &result.snippet, fetch_limit);
            fetched_count += 1;

            let snippet = if !result.snippet.is_empty() && result.snippet != text {
                Some(result.snippet.clone())
            } else {
                None
            };

            sources.push(Source {
                id: idx + 1,
                title: if title.is_empty() {
                    result.title.clone()
                } else {
                    title
                },
                url: result.url.clone(),
                snippet,
                content: text,
                truncated,
            });
        } else {
            failed_count += 1;
            let text = if result.snippet.is_empty() {
                "[Fetch failed]".to_string()
            } else {
                result.snippet.clone()
            };
            sources.push(Source {
                id: idx + 1,
                title: result.title.clone(),
                url: result.url.clone(),
                snippet: None,
                content: text,
                truncated: false,
            });
        }
    }

    // Tier-1 rerank: deterministic BM25
    // Compute scores upfront when adaptive mode is On or Auto.
    let (bm25_scores_opt, reranked) = match cfg.adaptive_budget {
        config::AdaptiveBudget::Off => {
            (None, utils::reranker::rerank_deterministic(&queries_list, &sources))
        }
        _ => {
            let (scores, reranked) =
                utils::reranker::rerank_with_scores(&queries_list, &sources);
            (Some(scores), reranked)
        }
    };
    sources = reranked;

    // Resolve Auto → On/Off via dominance ratio:
    //   dominance_ratio = (max_score / total_score) × N
    // If > 1.5 the top source would get 50%+ more than flat allocation → enable adaptive.
    let use_adaptive = match cfg.adaptive_budget {
        config::AdaptiveBudget::On => true,
        config::AdaptiveBudget::Off => false,
        config::AdaptiveBudget::Auto => bm25_scores_opt.as_ref().is_some_and(|scores| {
            let total: f64 = scores.iter().sum();
            let max: f64 = scores.iter().cloned().fold(0.0_f64, f64::max);
            let n = scores.len() as f64;
            total > 0.0 && (max / total * n) > 1.5
        }),
    };

    if use_adaptive {
        let bm25_scores = bm25_scores_opt.unwrap();
        let total_budget = cfg.max_query_budget;
        let total_score: f64 = bm25_scores.iter().sum();
        let mut allocs: Vec<usize> = if total_score > 0.0 {
            bm25_scores
                .iter()
                .map(|&s| {
                    (s / total_score * total_budget as f64)
                        .round()
                        .max(200.0)
                        .min(fetch_limit as f64) as usize
                })
                .collect()
        } else {
            vec![total_budget / sources.len().max(1); sources.len()]
        };

        allocs = utils::reranker::redistribute_budget(&sources, &allocs, &bm25_scores);

        for (source, &alloc) in sources.iter_mut().zip(allocs.iter()) {
            if source.content.len() > alloc {
                source.content = source.content.chars().take(alloc).collect();
                source.truncated = true;
            }
        }
    } else {
        for source in &mut sources {
            if source.content.len() > per_page_limit {
                source.content = source.content.chars().take(per_page_limit).collect();
                source.truncated = true;
            }
        }
    }

    // Tier-2 rerank: LLM-assisted (opt-in)
    #[cfg(feature = "llm")]
    if config.llm.enabled && config.llm.llm_rerank_enabled {
        let llm_client = llm::client::LlmClient::new(&config.llm);
        sources = utils::reranker::rerank_llm(&queries_list, &sources, &llm_client).await;
    }

    // Reassign IDs after reranking
    for (i, source) in sources.iter_mut().enumerate() {
        source.id = i + 1;
    }

    // Snippet pool: unread pages from reserve
    let snippet_pool: Vec<SnippetEntry> = reserve_pool
        .iter()
        .enumerate()
        .map(|(i, r)| SnippetEntry {
            id: sources.len() + i + 1,
            title: r.title.clone(),
            url: r.url.clone(),
            snippet: r.snippet.clone(),
        })
        .collect();

    let total_chars: usize = sources.iter().map(|s| s.content.len()).sum();
    let raw_bytes: usize = fetch_timing.values().map(|(_, b)| b).sum();

    // LLM summarization
    #[cfg(feature = "llm")]
    let (summary, llm_summary_error) = if config.llm.enabled && config.llm.summarization_enabled {
        let llm_client = llm::client::LlmClient::new(&config.llm);
        let max_words = if config.llm.max_summary_words > 0 {
            config.llm.max_summary_words
        } else {
            cfg.max_query_budget / 5
        };
        match llm::summarizer::summarize_results(&queries_list, &sources, &llm_client, max_words)
            .await
        {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e.to_string())),
        }
    } else {
        (None, None)
    };

    #[cfg(not(feature = "llm"))]
    let (summary, llm_summary_error) = (None::<String>, None::<String>);

    Ok(QueryResult {
        queries: queries_list,
        sources,
        snippet_pool,
        stats: Stats {
            fetched: fetched_count,
            failed: failed_count,
            gap_filled,
            total_chars,
            per_page_limit,
            num_results_per_query: nrpq,
            raw_bytes,
        },
        summary,
        llm_summary_error,
    })
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

/// Pipeline tests with LLM features enabled (require both `backends` + `llm` features).
#[cfg(test)]
#[cfg(all(feature = "backends", feature = "llm"))]
mod llm_pipeline_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn mock_config_with_llm(searxng_url: &str, llm_base_url: &str) -> Config {
        let mut config = Config::default();
        config.backends.searxng.url = searxng_url.to_string();
        config.server.max_result_length = 4000;
        config.server.max_query_budget = 16000;
        config.server.max_total_results = 5;
        config.server.search_timeout = 5;
        config.llm.enabled = true;
        config.llm.base_url = llm_base_url.to_string();
        config.llm.model = "test-model".to_string();
        config.llm.timeout = 5;
        config
    }

    #[tokio::test]
    async fn pipeline_with_query_expansion() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;
        let llm_server = MockServer::start().await;

        // LLM returns 2 expanded queries
        let llm_body = serde_json::json!({
            "choices": [{"message": {"content": "[\"rust async patterns\", \"tokio runtime tutorial\"]"}}]
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&llm_body))
            .mount(&llm_server)
            .await;

        // SearXNG returns results
        let page_url = format!("{}/page1", page_server.uri());
        let search_body = serde_json::json!({
            "results": [{"title": "Rust", "url": &page_url, "content": "Rust async"}]
        });
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><body><p>Rust async programming content.</p></body></html>",
            ))
            .mount(&page_server)
            .await;

        let mut config =
            mock_config_with_llm(&search_server.uri(), &format!("{}/v1", llm_server.uri()));
        config.llm.expansion_enabled = true;
        config.llm.summarization_enabled = false;
        config.llm.llm_rerank_enabled = false;

        let result = query(&["rust"], &config).await.unwrap();

        // Queries should be expanded (original + variants)
        assert!(result.queries.len() >= 1);
        assert_eq!(result.queries[0], "rust");
        assert!(result.sources.len() >= 1);
    }

    #[tokio::test]
    async fn pipeline_with_summarization() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;
        let llm_server = MockServer::start().await;

        // LLM returns a summary (called once for summarization, expansion disabled)
        let llm_body = serde_json::json!({
            "choices": [{"message": {"content": "## Summary\n\nRust is a systems language [1]."}}]
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&llm_body))
            .mount(&llm_server)
            .await;

        let page_url = format!("{}/page1", page_server.uri());
        let search_body = serde_json::json!({
            "results": [{"title": "Rust", "url": &page_url, "content": "Rust systems programming"}]
        });
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><body><p>Rust is a systems language.</p></body></html>",
            ))
            .mount(&page_server)
            .await;

        let mut config =
            mock_config_with_llm(&search_server.uri(), &format!("{}/v1", llm_server.uri()));
        config.llm.expansion_enabled = false;
        config.llm.summarization_enabled = true;
        config.llm.llm_rerank_enabled = false;

        let result = query(&["rust"], &config).await.unwrap();

        assert!(result.summary.is_some(), "summary should be present");
        let summary = result.summary.unwrap();
        assert!(summary.contains("Summary") || summary.contains("Rust"));
        assert!(result.llm_summary_error.is_none());
    }

    #[tokio::test]
    async fn pipeline_summarization_error_is_captured() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;
        let llm_server = MockServer::start().await;

        // LLM returns 500 error
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&llm_server)
            .await;

        let page_url = format!("{}/page1", page_server.uri());
        let search_body = serde_json::json!({
            "results": [{"title": "Test", "url": &page_url, "content": "content"}]
        });
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><body><p>Content.</p></body></html>",
            ))
            .mount(&page_server)
            .await;

        let mut config =
            mock_config_with_llm(&search_server.uri(), &format!("{}/v1", llm_server.uri()));
        config.llm.expansion_enabled = false;
        config.llm.summarization_enabled = true;
        config.llm.llm_rerank_enabled = false;

        let result = query(&["test"], &config).await.unwrap();

        // Pipeline succeeds but captures LLM error
        assert!(result.summary.is_none());
        assert!(result.llm_summary_error.is_some(), "should capture LLM error");
    }
}

#[cfg(test)]
#[cfg(feature = "backends")]
mod pipeline_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Build a minimal config pointing SearXNG at a mock server.
    fn mock_config(searxng_url: &str) -> Config {
        let mut config = Config::default();
        config.backends.searxng.url = searxng_url.to_string();
        config.server.max_result_length = 4000;
        config.server.max_query_budget = 16000;
        config.server.max_total_results = 5;
        config.server.search_timeout = 5;
        config
    }

    #[tokio::test]
    async fn pipeline_search_fetch_clean_rerank() {
        // 1. Mock SearXNG search engine
        let search_server = MockServer::start().await;
        // 2. Mock web pages to fetch
        let page_server = MockServer::start().await;

        let page_url_1 = format!("{}/page1", page_server.uri());
        let page_url_2 = format!("{}/page2", page_server.uri());

        // SearXNG returns 2 results
        let search_body = serde_json::json!({
            "results": [
                {"title": "Rust Programming", "url": &page_url_1, "content": "Learn Rust systems programming"},
                {"title": "Tokio Async", "url": &page_url_2, "content": "Async runtime for Rust"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        // Pages return HTML
        Mock::given(method("GET"))
            .and(path("/page1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><head><title>Rust Programming</title></head>\
                 <body><p>Rust is a systems programming language focused on safety.</p></body></html>",
            ))
            .mount(&page_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><head><title>Tokio Tutorial</title></head>\
                 <body><p>Tokio is an async runtime for Rust applications.</p></body></html>",
            ))
            .mount(&page_server)
            .await;

        let config = mock_config(&search_server.uri());
        let result = query(&["rust programming"], &config).await.unwrap();

        assert_eq!(result.queries, vec!["rust programming"]);
        assert_eq!(result.sources.len(), 2);
        assert_eq!(result.stats.fetched, 2);
        assert_eq!(result.stats.failed, 0);

        // Sources should have cleaned content
        assert!(!result.sources[0].content.is_empty());
        assert!(!result.sources[1].content.is_empty());

        // IDs should be 1-based after reranking
        assert_eq!(result.sources[0].id, 1);
        assert_eq!(result.sources[1].id, 2);
    }

    #[tokio::test]
    async fn pipeline_handles_fetch_failure() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;

        let page_url = format!("{}/good", page_server.uri());

        let search_body = serde_json::json!({
            "results": [
                {"title": "Good Page", "url": &page_url, "content": "Good snippet"},
                {"title": "Bad Page", "url": "http://192.0.2.1:1/nonexistent", "content": "Will fail"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/good"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><body><p>Good content here.</p></body></html>",
            ))
            .mount(&page_server)
            .await;

        let config = mock_config(&search_server.uri());
        let result = query(&["test"], &config).await.unwrap();

        assert_eq!(result.stats.fetched, 1);
        assert_eq!(result.stats.failed, 1);
        assert_eq!(result.sources.len(), 2);
    }

    #[tokio::test]
    async fn pipeline_deduplicates_urls() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;

        let page_url = format!("{}/page", page_server.uri());

        // Same URL appears twice in results
        let search_body = serde_json::json!({
            "results": [
                {"title": "Page", "url": &page_url, "content": "Snippet 1"},
                {"title": "Page Dup", "url": &page_url, "content": "Snippet 2"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><body><p>Page content</p></body></html>"),
            )
            .mount(&page_server)
            .await;

        let config = mock_config(&search_server.uri());
        let result = query(&["test"], &config).await.unwrap();

        // Should deduplicate to 1 source
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.stats.fetched, 1);
    }

    #[tokio::test]
    async fn pipeline_filters_binary_urls() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;

        let good_url = format!("{}/page", page_server.uri());

        let search_body = serde_json::json!({
            "results": [
                {"title": "Good", "url": &good_url, "content": "Good page"},
                {"title": "PDF", "url": "https://example.com/file.pdf", "content": "A PDF"},
                {"title": "ZIP", "url": "https://example.com/file.zip", "content": "A ZIP"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/page"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><body><p>Content</p></body></html>"),
            )
            .mount(&page_server)
            .await;

        let config = mock_config(&search_server.uri());
        let result = query(&["test"], &config).await.unwrap();

        // Binary URLs should be filtered out
        assert_eq!(result.sources.len(), 1);
        assert!(result.sources[0].url.contains("/page"));
    }

    #[tokio::test]
    async fn pipeline_multiple_queries_round_robin() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;

        let url1 = format!("{}/rust", page_server.uri());
        let url2 = format!("{}/tokio", page_server.uri());

        // Each query returns different results
        // Since both queries hit the same mock, return both results
        let search_body = serde_json::json!({
            "results": [
                {"title": "Rust", "url": &url1, "content": "Rust lang"},
                {"title": "Tokio", "url": &url2, "content": "Async Rust"},
            ]
        });

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        for p in ["/rust", "/tokio"] {
            Mock::given(method("GET"))
                .and(path(p))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_string(&format!("<html><body><p>{p} content</p></body></html>")),
                )
                .mount(&page_server)
                .await;
        }

        let config = mock_config(&search_server.uri());
        let result = query(&["rust", "async"], &config).await.unwrap();

        // Should have sources (deduped across queries)
        assert!(result.sources.len() >= 1);
        assert_eq!(result.queries, vec!["rust", "async"]);
    }

    #[tokio::test]
    async fn pipeline_unknown_backend_returns_error() {
        let config = Config::default();
        let result = query_with_options(&["test"], &config, None, None, Some("nonexistent")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown backend"));
    }

    #[tokio::test]
    async fn pipeline_empty_queries_returns_error() {
        let config = Config::default();
        let result = query(&[], &config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pipeline_snippet_pool_contains_reserves() {
        let search_server = MockServer::start().await;
        let page_server = MockServer::start().await;

        // Create many results to exceed max_total_results
        let mut results = Vec::new();
        for i in 0..8 {
            let url = format!("{}/page{i}", page_server.uri());
            results.push(serde_json::json!({
                "title": format!("Page {i}"),
                "url": url,
                "content": format!("Snippet {i}"),
            }));
        }

        let search_body = serde_json::json!({"results": results});

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
            .mount(&search_server)
            .await;

        for i in 0..8 {
            Mock::given(method("GET"))
                .and(path(format!("/page{i}")))
                .respond_with(ResponseTemplate::new(200).set_body_string(&format!(
                    "<html><body><p>Content for page {i}</p></body></html>"
                )))
                .mount(&page_server)
                .await;
        }

        let mut config = mock_config(&search_server.uri());
        config.server.max_total_results = 3;
        config.server.results_per_query = 3;
        config.server.oversampling_factor = 3; // oversample to get reserve pool

        let result = query(&["test"], &config).await.unwrap();

        // Should have max_total_results sources
        assert_eq!(result.sources.len(), 3);
        // Reserve pool should have the remaining
        assert!(result.snippet_pool.len() > 0, "snippet pool should have reserves");
    }
}

// ---------------------------------------------------------------------------
// clean() unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod clean_tests {
    use super::*;

    #[test]
    fn clean_returns_correct_fields() {
        let result = clean("<html><body><p>hello world</p></body></html>", 8000);
        assert!(
            result.text.contains("hello world"),
            "text should contain 'hello world', got: {}",
            result.text
        );
        assert_eq!(result.char_count, result.text.len());
        assert!(!result.truncated);
    }

    #[test]
    fn clean_truncated_flag() {
        let result = clean(
            "<html><body><p>hello world this is long content</p></body></html>",
            5,
        );
        assert!(result.truncated, "should be truncated with max_chars=5");
        assert!(
            result.char_count <= 5,
            "char_count ({}) should be <= 5",
            result.char_count
        );
    }

    #[test]
    fn clean_empty_html() {
        let result = clean("", 8000);
        assert!(
            result.text.is_empty(),
            "empty HTML should produce empty text, got: {:?}",
            result.text
        );
        assert_eq!(result.char_count, 0);
        assert!(!result.truncated);
    }

    #[test]
    fn clean_with_noise_elements() {
        let html = r#"<html><body>
            <nav>Navigation menu</nav>
            <script>alert('xss')</script>
            <p>Real content here</p>
            <footer>Footer stuff</footer>
        </body></html>"#;
        let result = clean(html, 8000);
        assert!(
            result.text.contains("Real content here"),
            "should keep real content"
        );
        assert!(
            !result.text.contains("alert"),
            "should strip script content"
        );
        assert!(
            !result.text.contains("Navigation menu"),
            "should strip nav content"
        );
        assert!(
            !result.text.contains("Footer stuff"),
            "should strip footer content"
        );
    }
}

// ---------------------------------------------------------------------------
// fetch() unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod fetch_tests {
    use super::*;

    #[tokio::test]
    async fn fetch_binary_url_rejected() {
        let cfg = Config::default();
        let result = fetch("https://example.com/file.pdf", &cfg).await;
        assert!(result.is_err(), "binary URL should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("binary file URL filtered"),
            "error should mention binary filter, got: {err}"
        );
    }

    #[tokio::test]
    async fn fetch_blocked_domain_rejected() {
        let mut cfg = Config::default();
        cfg.server.blocked_domains = vec!["blocked.example.com".to_string()];
        let result = fetch("https://blocked.example.com/page", &cfg).await;
        assert!(result.is_err(), "blocked domain should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("blocked by domain filter"),
            "error should mention domain filter, got: {err}"
        );
    }

    #[tokio::test]
    async fn fetch_allowed_domain_not_matching() {
        let mut cfg = Config::default();
        cfg.server.allowed_domains = vec!["allowed.example.com".to_string()];
        let result = fetch("https://other.example.com/page", &cfg).await;
        assert!(result.is_err(), "non-allowed domain should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("blocked by domain filter"),
            "error should mention domain filter, got: {err}"
        );
    }

    #[tokio::test]
    async fn fetch_returns_correct_fields() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test-page"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "<html><head><title>Test Title</title></head>\
                 <body><p>Test body content for fetching.</p></body></html>",
            ))
            .mount(&server)
            .await;

        let cfg = Config::default();
        let url = format!("{}/test-page", server.uri());
        let result = fetch(&url, &cfg).await.unwrap();

        assert_eq!(result.url, url);
        assert!(
            result.text.contains("Test body content"),
            "text should contain page body"
        );
        assert_eq!(result.char_count, result.text.len());
        assert!(
            result.title.contains("Test Title"),
            "title should be extracted"
        );
    }
}

// ---------------------------------------------------------------------------
// query_with_options() edge-case tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "backends")]
mod query_edge_tests {
    use super::*;

    #[tokio::test]
    async fn query_empty_queries_returns_error() {
        let cfg = Config::default();
        let result = query_with_options(&[], &cfg, None, None, None).await;
        assert!(result.is_err(), "empty queries should return an error");
    }
}
