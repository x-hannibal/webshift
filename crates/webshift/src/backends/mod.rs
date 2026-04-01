//! Search backend trait and implementations.
//!
//! Eight backends are available: SearXNG, Brave, Tavily, Exa, SerpAPI, Google,
//! Bing, and a generic HTTP backend. Each implements the [`SearchBackend`] trait.
//! Use [`create_backend`] to instantiate the default, or [`create_backend_by_name`]
//! to select one explicitly.

pub mod bing;
pub mod brave;
pub mod exa;
pub mod google;
pub mod http;
pub mod searxng;
pub mod serpapi;
pub mod tavily;

use crate::config::BackendsConfig;

/// A single search result from a backend.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Abstract search backend interface.
#[cfg_attr(docsrs, doc(cfg(feature = "backends")))]
#[async_trait::async_trait]
pub trait SearchBackend: Send + Sync {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>, crate::WebshiftError>;
}

/// Create a backend from config. Returns a boxed trait object.
#[cfg_attr(docsrs, doc(cfg(feature = "backends")))]
pub fn create_backend(
    config: &BackendsConfig,
) -> Result<Box<dyn SearchBackend>, crate::WebshiftError> {
    create_backend_by_name(&config.default, config)
}

/// Create a specific backend by name.
#[cfg_attr(docsrs, doc(cfg(feature = "backends")))]
pub fn create_backend_by_name(
    name: &str,
    config: &BackendsConfig,
) -> Result<Box<dyn SearchBackend>, crate::WebshiftError> {
    match name {
        "searxng" => Ok(Box::new(searxng::SearxngBackend::new(&config.searxng))),
        "brave" => Ok(Box::new(brave::BraveBackend::new(&config.brave)?)),
        "tavily" => Ok(Box::new(tavily::TavilyBackend::new(&config.tavily)?)),
        "exa" => Ok(Box::new(exa::ExaBackend::new(&config.exa)?)),
        "serpapi" => Ok(Box::new(serpapi::SerpapiBackend::new(&config.serpapi)?)),
        "google" => Ok(Box::new(google::GoogleBackend::new(&config.google)?)),
        "bing" => Ok(Box::new(bing::BingBackend::new(&config.bing)?)),
        "http" => Ok(Box::new(http::HttpBackend::new(&config.http)?)),
        other => Err(crate::WebshiftError::Backend(format!(
            "unknown backend: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BackendsConfig;

    #[test]
    fn create_backend_searxng() {
        let config = BackendsConfig::default(); // default is "searxng"
        let backend = create_backend(&config);
        assert!(backend.is_ok());
    }

    #[test]
    fn create_backend_unknown() {
        let mut config = BackendsConfig::default();
        config.default = "nonexistent".to_string();
        let result = create_backend(&config);
        assert!(result.is_err());
    }

    #[test]
    fn create_backend_brave_needs_api_key() {
        let mut config = BackendsConfig::default();
        config.default = "brave".to_string();
        let result = create_backend(&config);
        assert!(result.is_err());
    }

    #[test]
    fn create_backend_by_name_works() {
        let config = BackendsConfig::default();
        let backend = create_backend_by_name("searxng", &config);
        assert!(backend.is_ok());
    }

    #[test]
    fn create_backend_google_needs_api_key() {
        let mut config = BackendsConfig::default();
        config.default = "google".to_string();
        assert!(create_backend(&config).is_err());
    }

    #[test]
    fn create_backend_bing_needs_api_key() {
        let mut config = BackendsConfig::default();
        config.default = "bing".to_string();
        assert!(create_backend(&config).is_err());
    }

    #[test]
    fn create_backend_http_needs_url() {
        let mut config = BackendsConfig::default();
        config.default = "http".to_string();
        assert!(create_backend(&config).is_err());
    }
}
