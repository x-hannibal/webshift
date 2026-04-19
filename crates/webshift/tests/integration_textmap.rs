//! Integration tests for text-map feature.
//!
//! Run with: `cargo test -p webshift --features text-map`

#![cfg(feature = "text-map")]

use webshift::{extract_text_nodes, replace_text_nodes, TextReplacement};

fn fixture(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest}/tests/fixtures/{name}");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"))
}

#[test]
fn newsletter_roundtrip() {
    let html = fixture("newsletter.html");
    let map = extract_text_nodes(&html);
    assert!(map.nodes.len() > 5, "newsletter should have many text nodes");

    let replacements: Vec<TextReplacement> = map.nodes.iter()
        .map(|n| TextReplacement { id: n.id, text: n.text.to_uppercase() })
        .collect();
    let result = replace_text_nodes(&html, &replacements).unwrap();

    assert!(result.contains("<table"));
    assert!(result.contains("</table>"));
    assert!(result.contains(r#"href="https://example.com/unsubscribe?token=abc123"#));

    let result_map = extract_text_nodes(&result);
    for node in &result_map.nodes {
        assert_eq!(
            node.text,
            node.text.to_uppercase(),
            "node {} should be uppercase: {:?}",
            node.id,
            node.text
        );
    }
}

#[test]
fn selective_replace() {
    let html = fixture("newsletter.html");
    let map = extract_text_nodes(&html);

    let replacements: Vec<TextReplacement> = map.nodes.iter()
        .filter(|n| n.id % 2 == 0)
        .map(|n| TextReplacement { id: n.id, text: format!("[TRANSLATED] {}", n.text) })
        .collect();
    let result = replace_text_nodes(&html, &replacements).unwrap();
    let result_map = extract_text_nodes(&result);

    for node in &result_map.nodes {
        if node.id % 2 == 0 {
            assert!(
                node.text.starts_with("[TRANSLATED]"),
                "even node {} should be translated: {:?}",
                node.id,
                node.text
            );
        } else {
            let original = map.nodes.iter().find(|n| n.id == node.id).unwrap();
            assert_eq!(
                node.text, original.text,
                "odd node {} should be unchanged",
                node.id
            );
        }
    }
}

#[test]
fn attributes_untouched() {
    let html = fixture("attributes.html");
    let map = extract_text_nodes(&html);
    let replacements: Vec<TextReplacement> = map.nodes.iter()
        .map(|n| TextReplacement { id: n.id, text: "REPLACED".into() })
        .collect();
    let result = replace_text_nodes(&html, &replacements).unwrap();

    assert!(result.contains("utm_source=email"));
    assert!(result.contains("campaign=spring"));
    assert!(result.contains("cdn.example.com/img.png"));
    assert!(result.contains(r#"class="intro""#));
    assert!(result.contains(r#"id="p1""#));
    assert!(result.contains(r#"data-tracking="abc123""#));
    assert!(result.contains(r#"data-tooltip="Important info""#));
    assert!(result.contains("url('https://example.com/bg.jpg')"));
}

#[test]
fn noise_consistency() {
    let html = fixture("noise_heavy.html");
    let map = extract_text_nodes(&html);
    let extract_count = map.nodes.len();

    let replacements: Vec<TextReplacement> = map.nodes.iter()
        .map(|n| TextReplacement { id: n.id, text: format!("NODE_{}", n.id) })
        .collect();
    let result = replace_text_nodes(&html, &replacements).unwrap();

    let marker_count = (0..50)
        .filter(|i| result.contains(&format!("NODE_{i}")))
        .count();
    assert_eq!(
        extract_count, marker_count,
        "extract count ({extract_count}) must match replaceable count ({marker_count})"
    );
}

#[test]
fn fragmented_text() {
    let html = fixture("fragmented.html");
    let map = extract_text_nodes(&html);
    let texts: Vec<&str> = map.nodes.iter().map(|n| n.text.as_str()).collect();

    assert!(texts.contains(&"The quick"), "missing 'The quick', got: {texts:?}");
    assert!(texts.contains(&"brown fox"), "missing 'brown fox'");
    assert!(texts.contains(&"jumps over"), "missing 'jumps over'");
    assert!(texts.contains(&"the lazy dog."), "missing 'the lazy dog.'");
    assert!(texts.contains(&"Second paragraph as one piece."));
}
