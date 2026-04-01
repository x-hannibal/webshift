//! Live integration test for single-page fetch.
//!
//! Run with: `cargo test -p webshift -- --ignored`

#[tokio::test]
#[ignore]
async fn fetch_real_page() {
    let config = webshift::Config::default();
    let result = webshift::fetch("https://example.com", &config)
        .await
        .expect("fetch failed");

    assert!(!result.text.is_empty(), "expected non-empty text");
    assert!(result.char_count > 0, "expected char_count > 0");
    println!(
        "fetch_real_page: title={:?}, {} chars, truncated={}",
        result.title, result.char_count, result.truncated
    );
}
