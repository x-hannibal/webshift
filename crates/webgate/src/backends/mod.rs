//! Search backend trait and implementations.

pub mod searxng;
pub mod brave;
pub mod tavily;
pub mod exa;
pub mod serpapi;

/// A single search result from a backend.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Abstract search backend interface.
#[async_trait::async_trait]
pub trait SearchBackend: Send + Sync {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebgateError>;
}
