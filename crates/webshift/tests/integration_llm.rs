//! Live integration tests for LLM-augmented pipeline.
//!
//! These tests hit real LLM + search services configured in `test.toml`.
//! Run with: `cargo test -p webshift --features "backends llm" -- --ignored`

mod common;

use common::TestConfig;

fn load_or_skip() -> TestConfig {
    TestConfig::load().expect(
        "test.toml not found at workspace root — copy test.toml.example and configure",
    )
}

/// Requires: LLM enabled + at least one backend enabled.
fn require_llm_and_backend(tc: &TestConfig) -> Option<String> {
    if !tc.llm.enabled {
        println!("SKIP: LLM not enabled in test.toml");
        return None;
    }
    let backends = tc.enabled_backends();
    if backends.is_empty() {
        println!("SKIP: no backend enabled in test.toml");
        return None;
    }
    Some(backends[0].to_string())
}

#[tokio::test]
#[ignore]
async fn llm_expansion_live() {
    let tc = load_or_skip();
    let backend = match require_llm_and_backend(&tc) {
        Some(b) => b,
        None => return,
    };
    let mut config = tc.to_webshift_config(&backend);
    config.llm.expansion_enabled = true;
    config.llm.summarization_enabled = false;
    config.llm.llm_rerank_enabled = false;

    let result = webshift::query(&["quantum computing"], &config)
        .await
        .expect("query failed");

    println!("Expanded queries: {:?}", result.queries);
    assert!(
        result.queries.len() > 1,
        "expected LLM to expand queries, got: {:?}",
        result.queries
    );
    assert!(!result.sources.is_empty(), "expected sources");
}

#[tokio::test]
#[ignore]
async fn llm_summarization_live() {
    let tc = load_or_skip();
    let backend = match require_llm_and_backend(&tc) {
        Some(b) => b,
        None => return,
    };
    let mut config = tc.to_webshift_config(&backend);
    config.llm.expansion_enabled = false;
    config.llm.summarization_enabled = true;
    config.llm.llm_rerank_enabled = false;

    let result = webshift::query(&["rust async runtime"], &config)
        .await
        .expect("query failed");

    assert!(
        result.summary.is_some(),
        "expected LLM summary, got None (error: {:?})",
        result.llm_summary_error
    );
    let summary = result.summary.as_ref().unwrap();
    println!(
        "Summary ({} words, {} chars):\n{}",
        summary.split_whitespace().count(),
        summary.len(),
        summary
    );
    assert!(summary.len() > 50, "summary too short: {}", summary.len());
}

#[tokio::test]
#[ignore]
async fn llm_full_pipeline_live() {
    let tc = load_or_skip();
    let backend = match require_llm_and_backend(&tc) {
        Some(b) => b,
        None => return,
    };
    let mut config = tc.to_webshift_config(&backend);
    config.llm.expansion_enabled = true;
    config.llm.summarization_enabled = true;
    config.llm.llm_rerank_enabled = true;

    let result = webshift::query(&["benefits of functional programming"], &config)
        .await
        .expect("query failed");

    println!("Queries: {:?}", result.queries);
    println!(
        "Sources: {} fetched, {} failed",
        result.stats.fetched, result.stats.failed
    );
    for s in &result.sources {
        println!(
            "  [{}] {} ({} chars, truncated={})",
            s.id,
            s.title,
            s.content.len(),
            s.truncated
        );
    }
    if let Some(ref summary) = result.summary {
        println!(
            "Summary: {} words",
            summary.split_whitespace().count()
        );
    }
    if let Some(ref err) = result.llm_summary_error {
        println!("Summary error: {err}");
    }

    assert!(result.queries.len() > 1, "expansion should produce >1 query");
    assert!(!result.sources.is_empty(), "should have sources");
}
