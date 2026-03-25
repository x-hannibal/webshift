//! Two-tier result re-ranking.
//!
//! Tier 1 (always active): deterministic BM25 keyword overlap — zero cost, no LLM.
//! Tier 2 (opt-in, M4): LLM-assisted relevance scoring via a configured LLM client.
//!
//! Pipeline position in query: clean → rerank → top-N → (summarizer) → output.

use std::collections::HashMap;

use crate::Source;

// ---------------------------------------------------------------------------
// Tokenization
// ---------------------------------------------------------------------------

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// BM25 scoring
// ---------------------------------------------------------------------------

fn bm25_scores(query_tokens: &[String], docs: &[String], k1: f64, b: f64) -> Vec<f64> {
    let n = docs.len();
    let tokenized: Vec<Vec<String>> = docs.iter().map(|d| tokenize(d)).collect();
    let avg_len = tokenized.iter().map(|t| t.len()).sum::<usize>() as f64 / n.max(1) as f64;

    let mut scores = Vec::with_capacity(n);
    for doc_tokens in &tokenized {
        // Term frequency map
        let mut tf_map: HashMap<&str, usize> = HashMap::new();
        for token in doc_tokens {
            *tf_map.entry(token.as_str()).or_insert(0) += 1;
        }
        let doc_len = doc_tokens.len() as f64;

        let mut score = 0.0;
        // Deduplicate query tokens
        let unique_terms: std::collections::HashSet<&str> =
            query_tokens.iter().map(|s| s.as_str()).collect();

        for term in unique_terms {
            let tf = *tf_map.get(term).unwrap_or(&0) as f64;
            let df = tokenized.iter().filter(|t| t.iter().any(|w| w == term)).count() as f64;
            let idf = ((n as f64 - df + 0.5) / (df + 0.5) + 1.0).ln();
            let numerator = tf * (k1 + 1.0);
            let denominator = tf + k1 * (1.0 - b + b * doc_len / avg_len.max(1.0));
            score += idf * numerator / denominator.max(1e-9);
        }
        scores.push(score);
    }

    scores
}

// ---------------------------------------------------------------------------
// Tier 1 — deterministic BM25
// ---------------------------------------------------------------------------

/// Build a BM25 document string from a source: title + snippet + first 3000 chars of content.
fn source_to_doc(source: &Source) -> String {
    let snippet = source.snippet.as_deref().unwrap_or("");
    let content_prefix: String = source.content.chars().take(3000).collect();
    format!("{} {} {}", source.title, snippet, content_prefix)
}

/// Rerank sources by BM25 score against the query. Always active.
///
/// Returns the reordered source list (original list is not mutated).
pub fn rerank_deterministic(queries: &[String], sources: &[Source]) -> Vec<Source> {
    if sources.len() <= 1 {
        return sources.to_vec();
    }

    let query_str = queries.join(" ");
    let query_tokens = tokenize(&query_str);
    let docs: Vec<String> = sources.iter().map(source_to_doc).collect();
    let scores = bm25_scores(&query_tokens, &docs, 1.5, 0.75);

    let mut indexed: Vec<(f64, usize)> = scores.into_iter().zip(0..).collect();
    indexed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    indexed.iter().map(|&(_, i)| sources[i].clone()).collect()
}

/// Rerank sources by BM25 score and return (scores_in_new_order, reordered_sources).
///
/// The scores variant is used for proportional budget allocation (adaptive_budget).
pub fn rerank_with_scores(queries: &[String], sources: &[Source]) -> (Vec<f64>, Vec<Source>) {
    if sources.len() <= 1 {
        return (vec![1.0; sources.len()], sources.to_vec());
    }

    let query_str = queries.join(" ");
    let query_tokens = tokenize(&query_str);
    let docs: Vec<String> = sources.iter().map(source_to_doc).collect();
    let scores = bm25_scores(&query_tokens, &docs, 1.5, 0.75);

    let mut indexed: Vec<(f64, usize)> = scores.into_iter().zip(0..).collect();
    indexed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let sorted_scores: Vec<f64> = indexed.iter().map(|&(s, _)| s).collect();
    let sorted_sources: Vec<Source> = indexed.iter().map(|&(_, i)| sources[i].clone()).collect();

    (sorted_scores, sorted_sources)
}

// ---------------------------------------------------------------------------
// Budget redistribution (adaptive budget)
// ---------------------------------------------------------------------------

/// Reclaim unused budget from short/failed sources and give it to hungry ones.
///
/// Iterates up to 5 rounds (converges in 1-2). Returns updated allocations.
pub fn redistribute_budget(
    sources: &[Source],
    allocs: &[usize],
    bm25_scores: &[f64],
) -> Vec<usize> {
    let mut allocs = allocs.to_vec();
    for _ in 0..5 {
        let mut surplus: usize = 0;
        let mut hungry_indices: Vec<usize> = Vec::new();

        for i in 0..sources.len() {
            let actual = sources[i].content.len();
            let alloc = allocs[i];
            if actual < alloc {
                surplus += alloc - actual;
                allocs[i] = actual;
            } else if actual > alloc {
                hungry_indices.push(i);
            }
        }

        if surplus == 0 || hungry_indices.is_empty() {
            break;
        }

        let hungry_score: f64 = hungry_indices.iter().map(|&i| bm25_scores[i]).sum();
        if hungry_score <= 0.0 {
            let share = surplus / hungry_indices.len();
            for &i in &hungry_indices {
                allocs[i] += share;
            }
        } else {
            for &i in &hungry_indices {
                allocs[i] += (bm25_scores[i] / hungry_score * surplus as f64) as usize;
            }
        }
    }
    allocs
}

// ---------------------------------------------------------------------------
// Tier 2 — LLM-assisted (opt-in, behind `llm` feature flag)
// ---------------------------------------------------------------------------

/// Rerank sources using an LLM relevance judgment.
///
/// The LLM receives only title + snippet + first 200 chars of content per
/// source (lightweight input) and returns a ranked list of source IDs.
///
/// Falls back to the input order on any error.
#[cfg(feature = "llm")]
pub async fn rerank_llm(
    queries: &[String],
    sources: &[Source],
    client: &crate::llm::client::LlmClient,
) -> Vec<Source> {
    use crate::llm::client::ChatMessage;

    if sources.len() <= 1 {
        return sources.to_vec();
    }

    let query_str = queries.join(" | ");

    let items: String = sources
        .iter()
        .map(|s| {
            let preview = s
                .snippet
                .as_deref()
                .filter(|sn| !sn.is_empty())
                .unwrap_or(&s.content);
            let preview = &preview[..preview.len().min(200)];
            format!("[{}] {} — {}", s.id, s.title, preview)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Rank the following search results by relevance to the query: \"{query_str}\"\n\
         Output only a JSON array of IDs in order from most to least relevant. \
         No explanation, no markdown.\n\nResults:\n{items}"
    );

    match client
        .chat(&[ChatMessage::user(prompt)], 0.0)
        .await
    {
        Ok(text) => {
            let text = {
                let t = text.trim();
                let t = t
                    .strip_prefix("```json")
                    .or_else(|| t.strip_prefix("```"))
                    .unwrap_or(t);
                t.strip_suffix("```").unwrap_or(t).trim()
            };

            if let Ok(ranked_ids) = serde_json::from_str::<Vec<serde_json::Value>>(text) {
                let id_to_source: std::collections::HashMap<usize, &Source> =
                    sources.iter().map(|s| (s.id, s)).collect();

                let mut reranked: Vec<Source> = ranked_ids
                    .iter()
                    .filter_map(|v| v.as_u64().map(|id| id as usize))
                    .filter_map(|id| id_to_source.get(&id).copied().cloned())
                    .collect();

                // Append any sources the LLM omitted
                let mentioned: std::collections::HashSet<usize> = ranked_ids
                    .iter()
                    .filter_map(|v| v.as_u64().map(|id| id as usize))
                    .collect();
                reranked.extend(sources.iter().filter(|s| !mentioned.contains(&s.id)).cloned());

                return reranked;
            }
            sources.to_vec()
        }
        Err(_) => sources.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_source(id: usize, title: &str, content: &str) -> Source {
        Source {
            id,
            title: title.to_string(),
            url: format!("https://example.com/{id}"),
            snippet: None,
            content: content.to_string(),
            truncated: false,
        }
    }

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello, World! This is a TEST.");
        assert_eq!(tokens, vec!["hello", "world", "this", "is", "a", "test"]);
    }

    #[test]
    fn bm25_prefers_matching_content() {
        let queries = vec!["rust".to_string(), "programming".to_string()];
        let docs = vec![
            "rust programming language is fast".to_string(),
            "python scripting language is easy".to_string(),
            "rust and rust programming tutorials".to_string(),
        ];
        let scores = bm25_scores(&queries, &docs, 1.5, 0.75);
        // Doc 0 and Doc 2 should score higher than Doc 1
        assert!(scores[0] > scores[1]);
        assert!(scores[2] > scores[1]);
    }

    #[test]
    fn rerank_deterministic_orders_by_relevance() {
        let sources = vec![
            make_source(1, "Python Guide", "learn python scripting basics"),
            make_source(2, "Rust Tutorial", "rust programming patterns and async"),
            make_source(3, "Java Intro", "java enterprise development spring"),
        ];
        let queries = vec!["rust".to_string(), "async".to_string()];
        let reranked = rerank_deterministic(&queries, &sources);
        assert_eq!(reranked[0].id, 2, "Rust source should be first");
    }

    #[test]
    fn rerank_single_source_unchanged() {
        let sources = vec![make_source(1, "Only One", "single source")];
        let queries = vec!["test".to_string()];
        let reranked = rerank_deterministic(&queries, &sources);
        assert_eq!(reranked.len(), 1);
        assert_eq!(reranked[0].id, 1);
    }

    #[test]
    fn rerank_with_scores_returns_both() {
        let sources = vec![
            make_source(1, "Alpha", "alpha content"),
            make_source(2, "Beta", "beta content"),
        ];
        let queries = vec!["alpha".to_string()];
        let (scores, reranked) = rerank_with_scores(&queries, &sources);
        assert_eq!(scores.len(), 2);
        assert_eq!(reranked.len(), 2);
        assert_eq!(reranked[0].id, 1, "Alpha source should rank first");
        assert!(scores[0] >= scores[1]);
    }

    #[cfg(feature = "llm")]
    #[tokio::test]
    async fn rerank_llm_reorders_by_llm_judgment() {
        use crate::config::LlmConfig;
        use crate::llm::client::LlmClient;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // LLM returns source 2 as most relevant
        let body = serde_json::json!({
            "choices": [{"message": {"content": "[2, 1, 3]"}}]
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let sources = vec![
            make_source(1, "Python Guide", "learn python basics"),
            make_source(2, "Rust Tutorial", "rust async programming"),
            make_source(3, "Java Intro", "java enterprise spring"),
        ];

        let config = LlmConfig {
            enabled: true,
            base_url: format!("{}/v1", mock_server.uri()),
            model: "test".to_string(),
            timeout: 5,
            ..Default::default()
        };
        let client = LlmClient::new(&config);
        let queries = vec!["rust".to_string()];
        let reranked = rerank_llm(&queries, &sources, &client).await;

        assert_eq!(reranked[0].id, 2, "LLM should place source 2 first");
        assert_eq!(reranked[1].id, 1);
        assert_eq!(reranked[2].id, 3);
    }

    #[cfg(feature = "llm")]
    #[tokio::test]
    async fn rerank_llm_falls_back_on_error() {
        use crate::config::LlmConfig;
        use crate::llm::client::LlmClient;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let sources = vec![
            make_source(1, "Alpha", "alpha content"),
            make_source(2, "Beta", "beta content"),
        ];

        let config = LlmConfig {
            enabled: true,
            base_url: format!("{}/v1", mock_server.uri()),
            model: "test".to_string(),
            timeout: 5,
            ..Default::default()
        };
        let client = LlmClient::new(&config);
        let reranked = rerank_llm(&["test".to_string()], &sources, &client).await;

        // Fallback: same order as input
        assert_eq!(reranked[0].id, 1);
        assert_eq!(reranked[1].id, 2);
    }

    #[test]
    fn tokenize_empty_string() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_only_punctuation() {
        let tokens = tokenize("!!!");
        assert!(tokens.is_empty());
    }

    #[test]
    fn rerank_deterministic_empty_sources() {
        let queries = vec!["test".to_string()];
        let sources: Vec<Source> = vec![];
        let reranked = rerank_deterministic(&queries, &sources);
        assert!(reranked.is_empty());
    }

    #[test]
    fn rerank_with_scores_empty_sources() {
        let queries = vec!["test".to_string()];
        let sources: Vec<Source> = vec![];
        let (scores, reranked) = rerank_with_scores(&queries, &sources);
        assert!(scores.is_empty());
        assert!(reranked.is_empty());
    }

    #[test]
    fn redistribute_budget_no_surplus() {
        // All sources fully consumed (actual == alloc) → no change
        let sources = vec![
            make_source(1, "A", &"a".repeat(500)),
            make_source(2, "B", &"b".repeat(500)),
        ];
        let allocs = vec![500, 500];
        let bm25 = vec![1.0, 1.0];
        let new_allocs = redistribute_budget(&sources, &allocs, &bm25);
        assert_eq!(new_allocs, vec![500, 500]);
    }

    #[test]
    fn redistribute_budget_empty_inputs() {
        let sources: Vec<Source> = vec![];
        let allocs: Vec<usize> = vec![];
        let bm25: Vec<f64> = vec![];
        let new_allocs = redistribute_budget(&sources, &allocs, &bm25);
        assert!(new_allocs.is_empty());
    }

    #[test]
    fn redistribute_budget_reclaims_surplus() {
        let sources = vec![
            make_source(1, "Short", "ab"),    // actual 2, alloc 1000
            make_source(2, "Long", &"x".repeat(2000)), // actual 2000, alloc 1000
        ];
        let allocs = vec![1000, 1000];
        let bm25 = vec![1.0, 1.0];
        let new_allocs = redistribute_budget(&sources, &allocs, &bm25);
        // Source 1 should shrink to 2, surplus given to source 2
        assert_eq!(new_allocs[0], 2);
        assert!(new_allocs[1] > 1000);
    }
}
