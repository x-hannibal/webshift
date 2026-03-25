//! `robot harness` — run the full webgate pipeline against real services
//! with verbose diagnostic output for tuning BM25, budget, reranking, etc.
//!
//! Reads `test.toml` from the workspace root (same file used by integration tests).

use serde::Deserialize;
use std::path::PathBuf;
use std::time::Instant;

// ── TestConfig (mirrors crates/webgate/tests/common/mod.rs) ─────────

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestConfig {
    pub server: TestServerConfig,
    pub backends: TestBackendsConfig,
    pub llm: TestLlmConfig,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            server: TestServerConfig::default(),
            backends: TestBackendsConfig::default(),
            llm: TestLlmConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestServerConfig {
    pub max_total_results: usize,
    pub max_query_budget: usize,
    pub search_timeout: u64,
    pub language: String,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            max_total_results: 5,
            max_query_budget: 16_000,
            search_timeout: 10,
            language: "en".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct TestBackendsConfig {
    #[allow(dead_code)]
    #[serde(default = "default_backend")]
    pub default: String,
    pub searxng: TestSearxng,
    pub brave: TestBrave,
    pub tavily: TestTavily,
    pub exa: TestExa,
    pub serpapi: TestSerpapi,
    pub google: TestGoogle,
    pub bing: TestBing,
    pub http: TestHttp,
}

fn default_backend() -> String {
    "searxng".into()
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestSearxng {
    pub enabled: bool,
    pub url: String,
}
impl Default for TestSearxng {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://localhost:4000".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestBrave {
    pub enabled: bool,
    pub api_key: String,
    pub safesearch: u8,
}
impl Default for TestBrave {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            safesearch: 1,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestTavily {
    pub enabled: bool,
    pub api_key: String,
    pub search_depth: String,
}
impl Default for TestTavily {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            search_depth: "basic".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestExa {
    pub enabled: bool,
    pub api_key: String,
    pub num_sentences: u32,
    #[serde(rename = "type")]
    pub search_type: String,
}
impl Default for TestExa {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            num_sentences: 3,
            search_type: "auto".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestSerpapi {
    pub enabled: bool,
    pub api_key: String,
    pub engine: String,
    pub gl: String,
    pub hl: String,
    pub safe: String,
}
impl Default for TestSerpapi {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            engine: "google".into(),
            gl: "us".into(),
            hl: "en".into(),
            safe: "off".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestGoogle {
    pub enabled: bool,
    pub api_key: String,
    pub cx: String,
}
impl Default for TestGoogle {
    fn default() -> Self {
        Self { enabled: false, api_key: String::new(), cx: String::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestBing {
    pub enabled: bool,
    pub api_key: String,
    pub market: String,
}
impl Default for TestBing {
    fn default() -> Self {
        Self { enabled: false, api_key: String::new(), market: "en-US".into() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestHttp {
    pub enabled: bool,
    pub url: String,
}
impl Default for TestHttp {
    fn default() -> Self {
        Self { enabled: false, url: String::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TestLlmConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout: u64,
    pub expansion_enabled: bool,
    pub summarization_enabled: bool,
    pub llm_rerank_enabled: bool,
    pub max_summary_words: usize,
    pub input_budget_factor: u32,
}
impl Default for TestLlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:11434/v1".into(),
            api_key: String::new(),
            model: "gemma3:27b".into(),
            timeout: 30,
            expansion_enabled: true,
            summarization_enabled: true,
            llm_rerank_enabled: false,
            max_summary_words: 0,
            input_budget_factor: 3,
        }
    }
}

// ── Loading & conversion ────────────────────────────────────────────

impl TestConfig {
    pub fn load() -> Option<Self> {
        let path = find_test_toml()?;
        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str(&content).ok()
    }

    pub fn enabled_backends(&self) -> Vec<&str> {
        let mut v = Vec::new();
        if self.backends.searxng.enabled {
            v.push("searxng");
        }
        if self.backends.brave.enabled {
            v.push("brave");
        }
        if self.backends.tavily.enabled {
            v.push("tavily");
        }
        if self.backends.exa.enabled {
            v.push("exa");
        }
        if self.backends.serpapi.enabled {
            v.push("serpapi");
        }
        if self.backends.google.enabled {
            v.push("google");
        }
        if self.backends.bing.enabled {
            v.push("bing");
        }
        if self.backends.http.enabled {
            v.push("http");
        }
        v
    }

    pub fn to_webgate_config(&self, backend: &str) -> webgate::Config {
        use webgate::config::*;

        Config {
            server: ServerConfig {
                max_total_results: self.server.max_total_results,
                max_query_budget: self.server.max_query_budget,
                search_timeout: self.server.search_timeout,
                language: self.server.language.clone(),
                ..ServerConfig::default()
            },
            backends: BackendsConfig {
                default: backend.to_string(),
                searxng: SearxngConfig {
                    url: self.backends.searxng.url.clone(),
                },
                brave: BraveConfig {
                    api_key: self.backends.brave.api_key.clone(),
                    safesearch: self.backends.brave.safesearch,
                },
                tavily: TavilyConfig {
                    api_key: self.backends.tavily.api_key.clone(),
                    search_depth: self.backends.tavily.search_depth.clone(),
                },
                exa: ExaConfig {
                    api_key: self.backends.exa.api_key.clone(),
                    num_sentences: self.backends.exa.num_sentences,
                    search_type: self.backends.exa.search_type.clone(),
                },
                serpapi: SerpapiConfig {
                    api_key: self.backends.serpapi.api_key.clone(),
                    engine: self.backends.serpapi.engine.clone(),
                    gl: self.backends.serpapi.gl.clone(),
                    hl: self.backends.serpapi.hl.clone(),
                    safe: self.backends.serpapi.safe.clone(),
                },
                google: GoogleConfig {
                    api_key: self.backends.google.api_key.clone(),
                    cx: self.backends.google.cx.clone(),
                },
                bing: BingConfig {
                    api_key: self.backends.bing.api_key.clone(),
                    market: self.backends.bing.market.clone(),
                },
                http: HttpBackendConfig {
                    url: self.backends.http.url.clone(),
                    ..HttpBackendConfig::default()
                },
            },
            llm: LlmConfig {
                enabled: self.llm.enabled,
                base_url: self.llm.base_url.clone(),
                api_key: self.llm.api_key.clone(),
                model: self.llm.model.clone(),
                timeout: self.llm.timeout,
                expansion_enabled: self.llm.expansion_enabled,
                summarization_enabled: self.llm.summarization_enabled,
                llm_rerank_enabled: self.llm.llm_rerank_enabled,
                max_summary_words: self.llm.max_summary_words,
                input_budget_factor: self.llm.input_budget_factor,
            },
        }
    }
}

fn find_test_toml() -> Option<PathBuf> {
    // robot is always run from the workspace root.
    let candidate = PathBuf::from("test.toml");
    if candidate.exists() {
        return Some(candidate);
    }
    // Fallback: walk up from CARGO_MANIFEST_DIR.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut dir = PathBuf::from(manifest);
        for _ in 0..3 {
            let c = dir.join("test.toml");
            if c.exists() {
                return Some(c);
            }
            if !dir.pop() {
                break;
            }
        }
    }
    None
}

// ── Helpers ─────────────────────────────────────────────────────────

fn box_header(title: &str) -> String {
    let inner = format!("  {title}  ");
    let w = inner.len();
    format!("┌{0}┐\n│{1}│\n└{0}┘", "─".repeat(w), inner)
}

// ── Harness runner ──────────────────────────────────────────────────

pub async fn run_harness(
    query: &str,
    backend_override: Option<&str>,
    num_results: Option<usize>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let tc = TestConfig::load().ok_or(
        "test.toml not found — copy test.toml.example to test.toml and configure",
    )?;

    let backend_name = match backend_override {
        Some(b) => b.to_string(),
        None => {
            let enabled = tc.enabled_backends();
            if enabled.is_empty() {
                return Err("no backends enabled in test.toml".into());
            }
            enabled[0].to_string()
        }
    };

    let config = tc.to_webgate_config(&backend_name);

    let llm_label = if config.llm.enabled {
        format!("ON ({})", config.llm.model)
    } else {
        "OFF".into()
    };

    // ── Brief header while pipeline runs ────────────────────────────
    eprintln!("webgate harness | query={query} | backend={backend_name} | llm={llm_label}");
    eprintln!("running pipeline...");

    // ── Run pipeline ────────────────────────────────────────────────
    let start = Instant::now();
    let result = webgate::query_with_options(
        &[query],
        &config,
        num_results,
        None,
        Some(&backend_name),
    )
    .await?;
    let elapsed = start.elapsed();

    // ── Compute BM25 scores for report ──────────────────────────────
    let (scores, _) = webgate::utils::reranker::rerank_with_scores(
        &result.queries,
        &result.sources,
    );
    let max_score = scores.iter().cloned().fold(0.0_f64, f64::max);

    // ── Budget allocation data ──────────────────────────────────────
    let total_budget = config.server.max_query_budget;
    let total_score: f64 = scores.iter().sum();

    const BAR_WIDTH: usize = 12;

    // ── Snippet pool (first, before full previews) ───────────────────
    if !result.snippet_pool.is_empty() {
        println!();
        println!("{}", box_header(&format!("SNIPPET POOL ({})", result.snippet_pool.len())));
        println!();
        for sp in &result.snippet_pool {
            let preview: String = sp.snippet.chars().take(80).collect();
            println!("[{:>2}] {}  --  {preview}", sp.id, sp.title);
        }
    }

    // ── Content previews ────────────────────────────────────────────
    let preview_chars = if verbose { 600 } else { 300 };
    println!();
    println!("{}", box_header(&format!("CONTENT PREVIEWS ({})", result.sources.len())));
    println!();
    for (i, s) in result.sources.iter().enumerate() {
        let preview: String = s.content.chars().take(preview_chars).collect();
        println!("[{}] {}", s.id, s.url);
        println!("TITLE  : {}", s.title);
        println!("PREVIEW: {preview}");
        if i + 1 < result.sources.len() {
            println!();
        }
    }

    // ── LLM summary ─────────────────────────────────────────────────
    if let Some(ref summary) = result.summary {
        let word_count = summary.split_whitespace().count();
        println!();
        println!("{}", box_header(&format!("LLM SUMMARY ({} chars, ~{} words)", summary.len(), word_count)));
        println!();
        for line in summary.lines() {
            println!("{line}");
        }
    }
    if let Some(ref err) = result.llm_summary_error {
        println!();
        println!("{}", box_header("LLM SUMMARY ERROR"));
        println!();
        println!("{err}");
    }

    // ====================================================================
    //  CONSOLIDATED REPORT  (at the bottom for easy reading)
    // ====================================================================
    println!();
    println!("{}", box_header("WEBGATE HARNESS REPORT"));
    println!();

    // ── Config ──────────────────────────────────────────────────────
    println!("{}", box_header("CONFIG"));
    println!();
    println!("query:            {query}");
    println!("backend:          {backend_name}");
    println!("language:         {}", config.server.language);
    println!("llm:              {llm_label}");
    println!("budget:           {total_budget} chars");
    println!("max results:      {}", config.server.max_total_results);
    println!("per-page limit:   {} chars", config.server.max_result_length);
    println!("oversampling:     {}x", config.server.oversampling_factor);
    println!("adaptive budget:  {}", config.server.adaptive_budget);
    println!();

    // ── Pipeline stats ──────────────────────────────────────────────
    println!("{}", box_header("PIPELINE"));
    println!();
    println!("time:             {:.2}s", elapsed.as_secs_f64());
    println!("queries:          {} {:?}", result.queries.len(), result.queries);
    println!("fetched:          {}", result.stats.fetched);
    println!("failed:           {}", result.stats.failed);
    println!("gap filled:       {}", result.stats.gap_filled);
    println!("per-page limit:   {} chars", result.stats.per_page_limit);
    println!("total chars:      {}", result.stats.total_chars);
    let utilization = if total_budget > 0 {
        (result.stats.total_chars as f64 / total_budget as f64 * 100.0).round() as usize
    } else {
        0
    };
    println!("budget usage:     {}%", utilization);
    println!();

    // ── Compression chain ───────────────────────────────────────────
    println!("{}", box_header("COMPRESSION"));
    println!();
    let raw_kb = result.stats.raw_bytes as f64 / 1024.0;
    let clean_kb = result.stats.total_chars as f64 / 1024.0;
    let raw_to_clean_pct = if result.stats.raw_bytes > 0 {
        (1.0 - result.stats.total_chars as f64 / result.stats.raw_bytes as f64) * 100.0
    } else {
        0.0
    };
    println!("raw download:     {raw_kb:.1} KB");
    println!("clean text:       {clean_kb:.1} KB  ({raw_to_clean_pct:.0}% reduction)");
    if let Some(ref summary) = result.summary {
        let summary_kb = summary.len() as f64 / 1024.0;
        let raw_to_summary_pct = if result.stats.raw_bytes > 0 {
            (1.0 - summary.len() as f64 / result.stats.raw_bytes as f64) * 100.0
        } else {
            0.0
        };
        println!("llm summary:      {summary_kb:.1} KB  ({raw_to_summary_pct:.0}% reduction)");
    }
    println!();

    // ── Sources table (one row per record) ──────────────────────────
    println!("{}", box_header(&format!("SOURCES ({})", result.sources.len())));
    println!();
    println!(
        "{:>3}  {:<6}  {:<bar_w$}  {:>6}  {:>6}  {:>5}",
        "id", "score", "bm25", "chars", "budget", "trunc",
        bar_w = BAR_WIDTH,
    );
    println!("{}", "─".repeat(46));
    for (s, &score) in result.sources.iter().zip(scores.iter()) {
        let budget_alloc = if total_score > 0.0 && config.server.adaptive_budget {
            (score / total_score * total_budget as f64).round() as usize
        } else {
            result.stats.per_page_limit
        };
        let filled = if max_score > 0.0 {
            (score / max_score * BAR_WIDTH as f64).round() as usize
        } else {
            0
        };
        let bar: String = format!(
            "{}{}",
            "█".repeat(filled),
            "░".repeat(BAR_WIDTH - filled),
        );
        println!(
            "[{:>1}]  {:>6.4}  {bar}  {:>6}  {:>6}  {:>5}",
            s.id, score, s.content.len(), budget_alloc,
            if s.truncated { "yes" } else { "no" },
        );
    }
    println!();

    Ok(())
}
