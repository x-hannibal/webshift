//! URL sanitization, deduplication, and binary extension filtering.
//!
//! Port of `../mcp-webshift/src/mcp_webshift/utils/url.py`.

use std::collections::HashSet;
use url::Url;

/// Tracking query parameters to strip from URLs.
const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "gclid",
    "fbclid",
    "msclkid",
    "mc_cid",
    "mc_eid",
];

/// File extensions that indicate binary/non-HTML content.
const BINARY_EXTENSIONS: &[&str] = &[
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
    ".zip", ".tar", ".gz", ".exe", ".dmg", ".iso", ".rar", ".7z",
    ".mp3", ".mp4", ".avi", ".mov",
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".svg", ".webp",
];

/// Remove tracking parameters and fragment from a URL.
pub fn sanitize_url(url_str: &str) -> String {
    let Ok(mut parsed) = Url::parse(url_str) else {
        return url_str.to_string();
    };

    let filtered: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    parsed.set_fragment(None);

    if filtered.is_empty() {
        parsed.set_query(None);
    } else {
        let qs = filtered
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        parsed.set_query(Some(&qs));
    }

    parsed.to_string()
}

/// Return `true` if the URL path ends with a binary file extension.
///
/// This check runs **before** any network request.
pub fn is_binary_url(url_str: &str) -> bool {
    let path = Url::parse(url_str)
        .map(|u| u.path().to_lowercase())
        .unwrap_or_else(|_| url_str.to_lowercase());
    BINARY_EXTENSIONS.iter().any(|ext| path.ends_with(ext))
}

/// Return `true` if the URL passes the domain filter.
///
/// - If `allowed` is non-empty: only URLs matching an allowed domain pass.
/// - Otherwise: URLs matching a blocked domain are rejected.
/// - If both empty: all URLs pass.
///
/// Subdomain matching: `"reddit.com"` in blocked also blocks `"www.reddit.com"`.
pub fn is_domain_allowed(url_str: &str, blocked: &[String], allowed: &[String]) -> bool {
    if blocked.is_empty() && allowed.is_empty() {
        return true;
    }
    let host = Url::parse(url_str)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_default();

    let matches = |domains: &[String]| -> bool {
        domains
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{}", d)))
    };

    if !allowed.is_empty() {
        matches(allowed)
    } else {
        !matches(blocked)
    }
}

/// Deduplicate URLs after sanitization, preserving order.
pub fn dedup_urls(urls: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut result: Vec<String> = Vec::new();
    for url in urls {
        let sanitized = sanitize_url(url);
        let key = sanitized.to_lowercase().trim_end_matches('/').to_string();
        if seen.insert(key) {
            result.push(sanitized);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_tracking_params() {
        let url = "https://example.com/page?utm_source=google&q=rust&utm_medium=cpc";
        let result = sanitize_url(url);
        assert!(result.contains("q=rust"));
        assert!(!result.contains("utm_source"));
        assert!(!result.contains("utm_medium"));
    }

    #[test]
    fn sanitize_strips_fragment() {
        let url = "https://example.com/page#section";
        let result = sanitize_url(url);
        assert!(!result.contains('#'));
    }

    #[test]
    fn sanitize_strips_all_known_tracking_params() {
        for param in TRACKING_PARAMS {
            let url = format!("https://example.com/?{}=x&keep=1", param);
            let result = sanitize_url(&url);
            assert!(!result.contains(param), "should strip {}", param);
            assert!(result.contains("keep=1"));
        }
    }

    #[test]
    fn sanitize_passthrough_on_invalid_url() {
        let bad = "not a url";
        assert_eq!(sanitize_url(bad), bad);
    }

    #[test]
    fn is_binary_detects_extensions() {
        assert!(is_binary_url("https://example.com/file.pdf"));
        assert!(is_binary_url("https://example.com/archive.zip"));
        assert!(is_binary_url("https://example.com/image.png"));
        assert!(is_binary_url("https://example.com/video.mp4"));
    }

    #[test]
    fn is_binary_passes_html_urls() {
        assert!(!is_binary_url("https://example.com/page"));
        assert!(!is_binary_url("https://example.com/article.html"));
        assert!(!is_binary_url("https://example.com/blog/post"));
    }

    #[test]
    fn is_domain_allowed_empty_lists() {
        assert!(is_domain_allowed("https://any.com/", &[], &[]));
    }

    #[test]
    fn is_domain_allowed_blocklist() {
        let blocked = vec!["reddit.com".to_string()];
        assert!(!is_domain_allowed("https://reddit.com/r/rust", &blocked, &[]));
        assert!(!is_domain_allowed("https://www.reddit.com/r/rust", &blocked, &[]));
        assert!(is_domain_allowed("https://example.com/", &blocked, &[]));
    }

    #[test]
    fn is_domain_allowed_allowlist() {
        let allowed = vec!["docs.rs".to_string()];
        assert!(is_domain_allowed("https://docs.rs/tokio", &[], &allowed));
        assert!(!is_domain_allowed("https://example.com/", &[], &allowed));
    }

    #[test]
    fn dedup_preserves_order_and_removes_duplicates() {
        let urls = vec![
            "https://example.com/a".to_string(),
            "https://example.com/b".to_string(),
            "https://example.com/a".to_string(), // duplicate
            "https://example.com/c".to_string(),
        ];
        let result = dedup_urls(&urls);
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("/a"));
        assert!(result[1].contains("/b"));
        assert!(result[2].contains("/c"));
    }

    #[test]
    fn dedup_normalizes_trailing_slash() {
        let urls = vec![
            "https://example.com/page/".to_string(),
            "https://example.com/page".to_string(),
        ];
        let result = dedup_urls(&urls);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn dedup_strips_tracking_before_comparing() {
        let urls = vec![
            "https://example.com/?utm_source=a".to_string(),
            "https://example.com/?utm_source=b".to_string(),
        ];
        let result = dedup_urls(&urls);
        assert_eq!(result.len(), 1, "same URL after sanitization = duplicate");
    }
}
