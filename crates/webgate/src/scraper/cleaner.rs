//! HTML cleaning pipeline: scraper/html5ever noise removal + regex text sterilization.
//!
//! Port of `../mcp-webgate/src/mcp_webgate/scraper/cleaner.py`.

use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Static noise tag set — same elements as Python _NOISE_XPATH
// ---------------------------------------------------------------------------

static NOISE_TAGS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "script", "style", "nav", "footer", "header", "aside", "form",
        "iframe", "noscript", "svg", "button", "input", "select", "textarea",
    ]
    .into_iter()
    .collect()
});

static TITLE_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("title").expect("valid selector"));

// ---------------------------------------------------------------------------
// Regex patterns (same as Python)
// ---------------------------------------------------------------------------

/// C0/C1 control codes, replacement char, zero-width and BiDi override chars.
static UNICODE_JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f-\x9f\u{fffd}\u{200b}-\u{200f}\u{202a}-\u{202e}\u{2066}-\u{2069}]+"
    ).expect("valid regex")
});

/// Collapsible whitespace: spaces, tabs, non-breaking spaces.
static WHITESPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t\u{00A0}]+").expect("valid regex"));

/// Navigation / boilerplate noise lines.
static NOISE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:menu|home|search|sign in|log in|sign up|register|subscribe|newsletter\
|account|profile|cart|checkout|buy now|shop|close|cancel|skip to content\
|next|previous|back to top|privacy policy|terms|cookie|copyright\
|all rights reserved|legal|contact us|help|support|faq|social|follow us\
|share|facebook|twitter|instagram|linkedin|youtube|advertisement\
|sponsored|promoted|related posts|read more|loading|posted by\
|written by|author|category|tags)$"
    ).expect("valid regex")
});

/// Short date-only lines (e.g. "01/12/2024" or "Jan 1, 2024").
static DATE_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{1,2}[/\-]\d{1,2}[/\-]\d{2,4}|\w{3} \d{1,2},? \d{4}")
        .expect("valid regex")
});

/// Collapse 3+ consecutive newlines to double newline.
static MULTI_NL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n{3,}").expect("valid regex"));

// ---------------------------------------------------------------------------
// Typography normalization
// ---------------------------------------------------------------------------

/// Replace typographic characters with plain ASCII equivalents.
/// Mirrors Python's `str.translate(_TYPOGRAPHY_TRANS)` table.
pub fn normalize_typography(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            // Smart / curly single quotes → apostrophe
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}'
            | '\u{2039}' | '\u{203a}' => out.push('\''),
            // Smart / curly double quotes → "
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}'
            | '\u{00ab}' | '\u{00bb}' => out.push('"'),
            // Dashes → spaced hyphen
            '\u{2014}' | '\u{2013}' | '\u{2012}' | '\u{2015}' => out.push_str(" - "),
            '\u{2011}' => out.push('-'),
            '\u{00ad}' => {} // soft hyphen — drop
            // Ellipsis
            '\u{2026}' => out.push_str("..."),
            // Typographic spaces → regular space
            '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}'
            | '\u{2007}' | '\u{2009}' | '\u{200a}' | '\u{202f}' => out.push(' '),
            // Ligatures
            '\u{fb00}' => out.push_str("ff"),
            '\u{fb01}' => out.push_str("fi"),
            '\u{fb02}' => out.push_str("fl"),
            '\u{fb03}' => out.push_str("ffi"),
            '\u{fb04}' => out.push_str("ffl"),
            '\u{fb05}' | '\u{fb06}' => out.push_str("st"),
            '\u{0132}' => out.push_str("IJ"),
            '\u{0133}' => out.push_str("ij"),
            '\u{0152}' => out.push_str("OE"),
            '\u{0153}' => out.push_str("oe"),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Stage 1: HTML → raw text
// ---------------------------------------------------------------------------

/// Strip noise HTML elements and extract plain text.
///
/// Uses scraper/html5ever (pure Rust). Semantically equivalent to the Python
/// lxml XPath approach — same tag set removed, same text extraction.
pub fn clean_html(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let doc = Html::parse_document(raw);
    let mut parts: Vec<String> = Vec::new();

    for node in doc.tree.nodes() {
        // Check if it's a text node
        if let scraper::node::Node::Text(text) = node.value() {
            let t: &str = text;
            let t = t.trim();
            if t.is_empty() {
                continue;
            }
            // Skip if any ancestor is a noise element
            let in_noise = node
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|el| NOISE_TAGS.contains(el.value().name()));
            if !in_noise {
                parts.push(t.to_string());
            }
        }
    }
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Stage 2: raw text → sterilized text
// ---------------------------------------------------------------------------

/// Sterilize extracted text: unicode junk, typography, noise lines, duplicates.
pub fn clean_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Unicode / BiDi sterilization
    let text = UNICODE_JUNK.replace_all(text, " ");
    let text = WHITESPACE.replace_all(&text, " ");

    // Typography normalization
    let text = normalize_typography(&text);

    // Collapse whitespace again (em-dash replacements may have added spaces)
    let text = WHITESPACE.replace_all(&text, " ");

    // Line-by-line noise filtering
    let mut cleaned: Vec<&str> = Vec::new();
    let mut prev = "";
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if NOISE_LINE.is_match(line) {
            continue;
        }
        if line.len() < 5 && !line.chars().any(|c| c.is_alphanumeric()) {
            continue;
        }
        if line.len() < 20 && DATE_ONLY.is_match(line) {
            continue;
        }
        if line == prev {
            continue;
        }
        cleaned.push(line);
        prev = line;
    }

    let result = cleaned.join("\n");
    MULTI_NL.replace_all(&result, "\n\n").into_owned()
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Extract the `<title>` element text from raw HTML.
pub fn extract_title(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let doc = Html::parse_document(raw);
    doc.select(&TITLE_SEL)
        .next()
        .map(|el| {
            let raw_title: String = el.text().collect();
            let normalized = normalize_typography(raw_title.trim());
            // Collapse whitespace: em-dash replacements may have added extra spaces
            normalized.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .unwrap_or_default()
}

/// Collect whole lines until the character budget is exhausted.
///
/// Returns `(windowed_text, truncated)`. Always cuts on a line boundary.
/// If the very first line exceeds the budget, hard-truncates to `max_chars`.
pub fn apply_window(text: &str, max_chars: usize) -> (String, bool) {
    if text.len() <= max_chars {
        return (text.to_string(), false);
    }
    let mut buf: Vec<&str> = Vec::new();
    let mut total = 0usize;
    for line in text.lines() {
        let needed = line.len() + if buf.is_empty() { 0 } else { 1 };
        if total + needed > max_chars {
            if buf.is_empty() {
                return (line[..max_chars].to_string(), true);
            }
            break;
        }
        buf.push(line);
        total += needed;
    }
    (buf.join("\n"), true)
}

/// Full cleaning pipeline for a single page.
///
/// Returns `(text, title, truncated)`.
pub fn process_page(raw_html: &str, snippet: &str, max_chars: usize) -> (String, String, bool) {
    let title = extract_title(raw_html);
    let mut text = clean_text(&clean_html(raw_html));

    // Heuristic: fall back to snippet if content is low-quality
    if text.is_empty()
        || (!snippet.is_empty() && text.len() < snippet.len())
        || text.matches('\u{fffd}').count() > 10
    {
        if !snippet.is_empty() {
            text = format!(
                "[Using search snippet - page content was low quality] {}",
                snippet
            );
        }
    }

    let (text, truncated) = apply_window(&text, max_chars);
    (text, title, truncated)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_noise_elements() {
        let html = r#"<html><body>
            <nav>Nav stuff</nav>
            <p>Hello world</p>
            <script>alert(1)</script>
            <footer>Footer</footer>
        </body></html>"#;
        let text = clean_html(html);
        assert!(text.contains("Hello world"), "should keep body text");
        assert!(!text.contains("Nav stuff"), "should strip nav");
        assert!(!text.contains("alert"), "should strip script");
        assert!(!text.contains("Footer"), "should strip footer");
    }

    #[test]
    fn extracts_title() {
        let html = r#"<html><head><title>Test Page</title></head><body>body</body></html>"#;
        assert_eq!(extract_title(html), "Test Page");
    }

    #[test]
    fn extracts_title_with_typography() {
        let html = "<html><head><title>Test \u{2014} Page</title></head></html>";
        assert_eq!(extract_title(html), "Test - Page");
    }

    #[test]
    fn empty_html_returns_empty() {
        assert_eq!(clean_html(""), "");
        assert_eq!(extract_title(""), "");
    }

    #[test]
    fn normalize_typography_handles_all_cases() {
        assert_eq!(normalize_typography("\u{2018}hello\u{2019}"), "'hello'");
        assert_eq!(normalize_typography("\u{201c}hi\u{201d}"), "\"hi\"");
        assert_eq!(normalize_typography("a\u{2014}b"), "a - b");
        assert_eq!(normalize_typography("\u{2026}"), "...");
        assert_eq!(normalize_typography("\u{fb01}le"), "file");
        assert_eq!(normalize_typography("\u{00ad}"), ""); // soft hyphen dropped
    }

    #[test]
    fn clean_text_filters_noise_lines() {
        let text = "menu\nHello world\nhome\nThis is content\nsign in";
        let result = clean_text(text);
        assert!(result.contains("Hello world"));
        assert!(result.contains("This is content"));
        assert!(!result.contains("menu"));
        assert!(!result.contains("home"));
        assert!(!result.contains("sign in"));
    }

    #[test]
    fn clean_text_deduplicates_lines() {
        let text = "line one\nline one\nline two";
        let result = clean_text(text);
        let count = result.lines().filter(|l| *l == "line one").count();
        assert_eq!(count, 1, "duplicate lines should be removed");
    }

    #[test]
    fn clean_text_strips_unicode_junk() {
        let text = "hello\x00world\u{200b}test";
        let result = clean_text(text);
        assert!(!result.contains('\x00'));
        assert!(!result.contains('\u{200b}'));
    }

    #[test]
    fn apply_window_respects_line_boundary() {
        let text = "line one\nline two\nline three";
        let (windowed, truncated) = apply_window(text, 10);
        assert!(truncated);
        assert!(!windowed.contains('\n') || windowed.ends_with("line one"));
        assert!(windowed.len() <= 10);
    }

    #[test]
    fn apply_window_no_truncation_when_fits() {
        let text = "short";
        let (windowed, truncated) = apply_window(text, 100);
        assert!(!truncated);
        assert_eq!(windowed, "short");
    }

    #[test]
    fn process_page_full_pipeline() {
        let html = r#"<html>
            <head><title>My Page</title></head>
            <body>
                <nav>skip this</nav>
                <p>This is the main content of the page.</p>
            </body>
        </html>"#;
        let (text, title, truncated) = process_page(html, "", 8000);
        assert_eq!(title, "My Page");
        assert!(text.contains("main content"));
        assert!(!text.contains("skip this"));
        assert!(!truncated);
    }

    #[test]
    fn process_page_falls_back_to_snippet() {
        let (text, _, _) = process_page("", "fallback snippet", 8000);
        assert!(text.contains("fallback snippet"));
    }
}
