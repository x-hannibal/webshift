//! Text-map: HTML text node replacement using lol_html streaming rewriter.

use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, text, EndTagHandler, RewriteStrSettings};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::WebshiftError;
use super::cleaner::TEXT_MAP_NOISE_SELECTOR;

/// Rebuild HTML with replaced text nodes.
///
/// Only nodes present in `replacements` are changed; all others are left
/// untouched. Tags, attributes, href, src, class, and style are never modified.
///
/// The node counter increments identically to `extract_text_nodes`: non-noise,
/// non-empty (trimmed) text nodes only — guaranteeing that IDs match across
/// the extract → replace round-trip.
pub fn replace_text_nodes(
    raw: &str,
    replacements: &[crate::TextReplacement],
) -> Result<String, WebshiftError> {
    let map: Rc<HashMap<usize, String>> = Rc::new(
        replacements.iter().map(|r| (r.id, r.text.clone())).collect(),
    );

    let noise_depth: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let counter: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    // Accumulates text from non-last chunks of the current text node.
    let pending: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    let nd_elem = Rc::clone(&noise_depth);
    let nd_text = Rc::clone(&noise_depth);
    let ctr = Rc::clone(&counter);
    let pnd = Rc::clone(&pending);
    let map_text = Rc::clone(&map);

    let output = rewrite_str(
        raw,
        RewriteStrSettings {
            element_content_handlers: vec![
                element!(TEXT_MAP_NOISE_SELECTOR, move |el| {
                    *nd_elem.borrow_mut() += 1;
                    let nd_inner = Rc::clone(&nd_elem);
                    if let Some(handlers) = el.end_tag_handlers() {
                        let handler: EndTagHandler<'static> = Box::new(move |_end| {
                            let v = *nd_inner.borrow();
                            if v > 0 {
                                *nd_inner.borrow_mut() -= 1;
                            }
                            Ok(())
                        });
                        handlers.push(handler);
                    }
                    Ok(())
                }),
                text!("*", move |chunk| {
                    // Skip text inside noise elements.
                    if *nd_text.borrow() > 0 {
                        return Ok(());
                    }

                    let chunk_str = chunk.as_str().to_string();

                    if chunk.last_in_text_node() {
                        // Assemble the full text node (previous chunks + this one).
                        let mut acc = pnd.borrow_mut();
                        acc.push_str(&chunk_str);
                        let full = acc.clone();
                        acc.clear();
                        drop(acc);

                        let trimmed = full.trim();
                        if trimmed.is_empty() {
                            // Whitespace-only node — remove it (matches extract logic).
                            chunk.remove();
                        } else {
                            let id = {
                                let mut c = ctr.borrow_mut();
                                let id = *c;
                                *c += 1;
                                id
                            };
                            if let Some(new_text) = map_text.get(&id) {
                                chunk.replace(new_text, ContentType::Text);
                            } else {
                                // No replacement: restore the full original text
                                // (previous chunks were already removed).
                                chunk.replace(&full, ContentType::Text);
                            }
                        }
                    } else {
                        // Non-last chunk: accumulate and remove from stream;
                        // the last chunk will re-emit the full original if needed.
                        pnd.borrow_mut().push_str(&chunk_str);
                        chunk.remove();
                    }

                    Ok(())
                }),
            ],
            ..RewriteStrSettings::new()
        },
    )
    .map_err(|e| WebshiftError::Parse(e.to_string()))?;

    Ok(output)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextReplacement;

    fn extract(html: &str) -> (Vec<crate::TextNode>, String) {
        super::super::cleaner::extract_text_nodes(html)
    }

    #[test]
    fn replace_simple() {
        let html = include_str!("../../tests/fixtures/simple.html");
        let replacements = vec![TextReplacement { id: 0, text: "Benvenuto".into() }];
        let result = replace_text_nodes(html, &replacements).unwrap();
        assert!(result.contains("Benvenuto"));
        assert!(!result.contains(">Welcome<"));
    }

    #[test]
    fn replace_preserves_attributes() {
        let html = include_str!("../../tests/fixtures/attributes.html");
        let (nodes, _) = extract(html);
        let replacements: Vec<TextReplacement> = nodes
            .iter()
            .map(|n| TextReplacement { id: n.id, text: "REPLACED".into() })
            .collect();
        let result = replace_text_nodes(html, &replacements).unwrap();
        assert!(
            result.contains("utm_source=email")
                || result.contains(r#"href="https://example.com/page?utm_source=email"#)
        );
        assert!(result.contains(r#"src="https://cdn.example.com/img.png""#));
        assert!(result.contains(r#"class="intro""#));
        assert!(result.contains(r#"data-tracking="abc123""#));
        assert!(result.contains(r#"style="color: blue;""#));
    }

    #[test]
    fn replace_preserves_structure() {
        let html = include_str!("../../tests/fixtures/simple.html");
        let replacements = vec![TextReplacement { id: 0, text: "Changed".into() }];
        let result = replace_text_nodes(html, &replacements).unwrap();
        assert!(result.contains("<h1>"));
        assert!(result.contains("</h1>"));
        assert!(result.contains("<strong>"));
        assert!(result.contains("</strong>"));
    }

    #[test]
    fn replace_partial() {
        let html = include_str!("../../tests/fixtures/simple.html");
        let replacements = vec![TextReplacement { id: 0, text: "Hola".into() }];
        let result = replace_text_nodes(html, &replacements).unwrap();
        assert!(result.contains("Hola"));
        assert!(result.contains("This is the first paragraph."));
    }

    #[test]
    fn replace_empty_replacements() {
        let html = include_str!("../../tests/fixtures/simple.html");
        let result = replace_text_nodes(html, &[]).unwrap();
        let (original_nodes, _) = extract(html);
        let (result_nodes, _) = extract(&result);
        assert_eq!(original_nodes.len(), result_nodes.len());
        for (o, r) in original_nodes.iter().zip(result_nodes.iter()) {
            assert_eq!(o.text, r.text);
        }
    }

    #[test]
    fn replace_noise_nodes_untouched() {
        let html = include_str!("../../tests/fixtures/noise_heavy.html");
        let replacements = vec![TextReplacement { id: 0, text: "REPLACED".into() }];
        let result = replace_text_nodes(html, &replacements).unwrap();
        assert!(result.contains("tracking"));
        assert!(result.contains("Home"));
    }

    #[test]
    fn replace_unicode() {
        let html = include_str!("../../tests/fixtures/multilingual.html");
        let replacements = vec![
            TextReplacement { id: 0, text: "Translated to Chinese: 你好世界".into() },
            TextReplacement { id: 3, text: "ترجمة عربية جديدة".into() },
        ];
        let result = replace_text_nodes(html, &replacements).unwrap();
        assert!(result.contains("你好世界"));
        assert!(result.contains("ترجمة عربية جديدة"));
    }

    #[test]
    fn roundtrip_identity() {
        let html = include_str!("../../tests/fixtures/newsletter.html");
        let (nodes, _) = extract(html);
        let replacements: Vec<TextReplacement> = nodes
            .iter()
            .map(|n| TextReplacement { id: n.id, text: n.text.clone() })
            .collect();
        let result = replace_text_nodes(html, &replacements).unwrap();
        let (result_nodes, _) = extract(&result);
        assert_eq!(nodes.len(), result_nodes.len());
        for (o, r) in nodes.iter().zip(result_nodes.iter()) {
            assert_eq!(
                o.text, r.text,
                "roundtrip changed node {}: {:?} → {:?}",
                o.id, o.text, r.text
            );
        }
    }
}
