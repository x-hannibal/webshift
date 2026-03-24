# PLAN.RS.md — Rust Port of mcp-webgate

> **Repo:** `annibale-x/webgate`
> **Crates:** `webgate` (library) + `webgate-mcp` (MCP server binary)
> **Target:** Standalone native binary for AI agents — zero runtime dependencies.

---

## 1. Rationale

The Python version of mcp-webgate works and is feature-complete (Phase 1–4). The Rust
port addresses a different set of problems:

- **Distribution:** A single static binary replaces the Python+pip+venv toolchain.
  Agent config becomes `{ "command": "mcp-webgate" }` — nothing else to install.
- **Embedding:** Agents written in Rust can call `webgate::fetch()` and `webgate::query()`
  in-process, without spawning a subprocess or speaking JSON-RPC.
- **Containers:** A scratch Docker image with one binary (~8 MB) replaces ~200 MB Python images.
- **CI/Edge:** No interpreter setup step. Copy binary, run.
- **Composability:** Other Rust crates can depend on `webgate` and build on top of it
  (aggregators, proxy MCP servers, agent toolkits).

The Python version continues to exist on PyPI as `mcp-webgate`. This is not a replacement,
it is a native alternative.

---

## 2. Workspace layout

```
webgate/                            # repo root
├── Cargo.toml                      # workspace definition
├── CLAUDE.md
├── PLAN.md
├── README.md
├── webgate.toml.example            # example config
│
├── crates/
│   ├── webgate/                    # library crate (crates.io: webgate)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # public API: fetch(), query(), Config
│   │       ├── config.rs           # serde config (toml + env + CLI)
│   │       ├── scraper/
│   │       │   ├── mod.rs
│   │       │   ├── fetcher.rs      # reqwest concurrent fetcher, streaming, UA rotation
│   │       │   └── cleaner.rs      # libxml2 HTML cleaning + regex text sterilization
│   │       ├── backends/
│   │       │   ├── mod.rs          # SearchBackend trait + SearchResult
│   │       │   ├── searxng.rs
│   │       │   ├── brave.rs
│   │       │   ├── tavily.rs
│   │       │   ├── exa.rs
│   │       │   └── serpapi.rs
│   │       ├── llm/
│   │       │   ├── mod.rs
│   │       │   ├── client.rs       # OpenAI-compatible async client (reqwest)
│   │       │   ├── expander.rs     # query expansion
│   │       │   └── summarizer.rs   # Markdown report with citations
│   │       └── utils/
│   │           ├── mod.rs
│   │           ├── url.rs          # sanitize, dedup, binary filter, domain filter
│   │           └── reranker.rs     # BM25 deterministic + LLM reranking
│   │
│   └── webgate-mcp/                # binary crate (crates.io: webgate-mcp)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs             # MCP server: tool registration, stdio transport
│                                   # [[bin]] name = "mcp-webgate"
│
├── tests/                          # integration tests
│   ├── test_cleaner.rs
│   ├── test_fetcher.rs
│   ├── test_backends.rs
│   └── test_pipeline.rs
│
└── .github/
    └── workflows/
        ├── ci.yml                  # test on push (ubuntu, windows, macos)
        └── release.yml             # cross-compile 5 targets, GitHub Release
```

---

## 3. Dependency map

### `webgate` (library)

| Python | Rust crate | Purpose | Notes |
|--------|-----------|---------|-------|
| `lxml` | **`libxml`** (0.3.8) | HTML parsing + XPath | Bindings to libxml2. Same engine, same XPath expressions as Python. See §4. |
| `httpx` | **`reqwest`** + **`tokio`** | Async HTTP, streaming, connection pool | `reqwest` stream API mirrors httpx `client.stream()` pattern |
| `pydantic` + `tomllib` | **`serde`** + **`toml`** | Config deserialization | Derive-based, zero boilerplate |
| `re` | **`regex`** | Unicode/BiDi sterilization, noise line filter | Same patterns, better performance |
| — | **`rand`** | UA rotation | `rand::seq::SliceRandom::choose()` |
| `urllib.parse` | **`url`** | URL parsing, sanitization | `url::Url` for parse/rebuild |
| — | **`serde_json`** | JSON serialization of tool output | |
| — | **`thiserror`** | Error types | |
| — | **`tracing`** | Structured logging (debug/trace) | |

### `webgate-mcp` (binary)

| Dependency | Purpose |
|-----------|---------|
| `webgate` | The library |
| **`rmcp`** or MCP SDK | MCP server, stdio transport, tool registration |
| **`clap`** | CLI argument parsing |
| **`tokio`** (full) | Async runtime |

### Build-time / system

| Dependency | When | How |
|-----------|------|-----|
| `libxml2` | `cargo build` / `cargo run` | System package: `apt install libxml2-dev` / `brew install libxml2` / `vcpkg install libxml2:x64-windows` |
| `libxml2` (static) | CI release builds | Static linking via `vcpkg` or bundled source. Produces self-contained binary. |
| `pkg-config` | Build time | Used by `libxml` crate to locate libxml2 |

---

## 4. HTML cleaning: libxml2 bindings rationale

The Python cleaner uses lxml (Python bindings to libxml2) with a single XPath expression:

```python
_NOISE_XPATH = (
    "//script | //style | //nav | //footer | //header"
    " | //aside | //form | //iframe | //noscript"
    " | //svg | //button | //input | //select | //textarea"
)
```

The `libxml` Rust crate wraps the **same C library** (libxml2). This means:

- **Same XPath expression** ports verbatim — no rewriting to CSS selectors
- **Same HTML parser** — identical parse tree, identical edge-case behavior
- **Same `text_content()` equivalent** — `node.get_content()` in Rust
- **Same `drop_tree()` equivalent** — `node.unlink_node()` in Rust

Example Rust port:

```rust
use libxml::parser::Parser;
use libxml::xpath::Context;

pub fn clean_html(raw: &str) -> String {
    let parser = Parser::default_html();
    let doc = match parser.parse_string(raw) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    let ctx = match Context::new(&doc) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let xpath = "//script|//style|//nav|//footer|//header\
                 |//aside|//form|//iframe|//noscript\
                 |//svg|//button|//input|//select|//textarea";

    if let Ok(result) = ctx.evaluate(xpath) {
        for node in result.get_nodes_as_vec() {
            node.unlink_node();
        }
    }

    doc.get_root_element()
        .map(|root| root.get_content())
        .unwrap_or_default()
}
```

Alternatives considered and rejected:

| Crate | Why not |
|-------|---------|
| `scraper` | CSS selectors only — would require rewriting all XPath. No `text_content()` equivalent built in. |
| `lol_html` | Streaming rewriter (Cloudflare) — great for transforms, poor for "parse → query → extract text" pattern. No XPath. |
| `html5ever` | Low-level tokenizer/tree builder. No XPath. Would require building our own query layer. |
| `select.rs` | CSS selectors only, less maintained than `scraper`. |

---

## 5. Component-by-component port guide

### 5.1 `config.rs` — Configuration system

**Python reference:** `config.py` (358 lines)

Port the three-level config resolution: **CLI args > env vars > `webgate.toml` > defaults**.

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub backends: BackendsConfig,
    pub llm: LlmConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ServerConfig {
    pub max_download_mb: u32,        // default 1
    pub max_result_length: usize,    // default 8000
    pub search_timeout: u64,         // default 8
    pub oversampling_factor: u32,    // default 2
    pub auto_recovery_fetch: bool,   // default false
    pub max_total_results: usize,    // default 20
    pub max_query_budget: usize,     // default 32000
    pub max_search_queries: usize,   // default 5
    pub results_per_query: usize,    // default 5
    pub blocked_domains: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub debug: bool,
    pub log_file: String,
    pub trace: bool,
    pub adaptive_budget: bool,
    pub adaptive_budget_fetch_factor: u32, // default 3
}
```

**Resolution chain:**
1. Parse `webgate.toml` with `toml::from_str()` into `Config`
2. Walk `WEBGATE_*` env vars, override matching fields
3. Parse CLI with `clap`, override non-None values

Use `clap` derive macros with `Option<T>` for all fields so "not provided" is distinguishable
from "provided as default value".

### 5.2 `scraper/fetcher.rs` — Concurrent HTTP fetcher

**Python reference:** `fetcher.py` (156 lines)

Key behaviors to preserve:
- **Streaming download** with `max_download_mb` hard cap — read body in chunks, abort when exceeded
- **UA rotation** — pick random User-Agent per request from the 40-entry list
- **Retry with backoff** on 429/502/503 — up to 2 retries with delays [1.0s, 2.5s]
- **Respect `Retry-After` header** — use it as minimum delay
- **Concurrent fetch** via `tokio::spawn` + `futures::future::join_all`
- **Connection limits** — max 10 connections, 5 keepalive

```rust
pub async fn fetch_urls(
    urls: &[String],
    max_bytes: usize,
    timeout_secs: u64,
) -> (HashMap<String, String>, HashMap<String, (f64, usize)>)
```

`reqwest` streaming pattern:

```rust
let mut stream = response.bytes_stream();
let mut body = Vec::new();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    body.extend_from_slice(&chunk);
    if body.len() > max_bytes {
        break;
    }
}
```

### 5.3 `scraper/cleaner.rs` — HTML cleaning + text sterilization

**Python reference:** `cleaner.py` (208 lines)

Two-stage pipeline (same as Python):

**Stage 1: `clean_html(raw) -> String`**
- Parse HTML with `Parser::default_html()`
- Remove noise nodes via XPath (see §4)
- Extract text via `get_content()`

**Stage 2: `clean_text(text) -> String`**
- Unicode/BiDi sterilization regex (same `_UNICODE_JUNK` pattern)
- Typography normalization table (smart quotes → ASCII, dashes, ligatures)
- Whitespace collapse
- Line-by-line noise filtering (`_NOISE_LINE` regex)
- Short-line / date-only filtering
- Duplicate line removal
- Collapse 3+ newlines to double

**Additional functions:**
- `extract_title(raw) -> String` — XPath `.//title`
- `apply_window(text, max_chars) -> (String, bool)` — line-boundary truncation
- `process_page(raw, snippet, max_chars) -> (String, String, bool)` — full pipeline

The typography normalization table (88 entries) maps directly to a `HashMap<char, &str>`
or a match expression. The regex patterns port verbatim to the `regex` crate.

### 5.4 `backends/mod.rs` — Search backend trait

**Python reference:** `base.py` (30 lines)

```rust
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[async_trait]
pub trait SearchBackend: Send + Sync {
    async fn search(
        &self,
        query: &str,
        num_results: usize,
        lang: Option<&str>,
    ) -> Result<Vec<SearchResult>>;
}
```

### 5.5 Backend implementations

**Python reference:** `searxng.py` (48 lines), `brave.py` (59 lines), `tavily.py`, `exa.py`, `serpapi.py`

Each backend is a struct holding its config + a `reqwest::Client`. All follow the same
pattern: build params → GET/POST → parse JSON → map to `Vec<SearchResult>`.

These are the simplest components to port — pure HTTP + JSON deserialization.

### 5.6 `utils/url.rs` — URL utilities

**Python reference:** `url.py` (98 lines)

- `sanitize_url()` — strip tracking params (utm_*, gclid, fbclid, etc.)
- `is_binary_url()` — check extension against blocklist (.pdf, .zip, .docx, etc.)
- `is_domain_allowed()` — blocklist/allowlist with subdomain matching
- `dedup_urls()` — sanitize + lowercase + dedup preserving order

Use the `url` crate for parsing. The tracking params set and binary extensions list
are static arrays.

### 5.7 `utils/reranker.rs` — BM25 + LLM reranking

**Python reference:** `reranker.py` (164 lines)

**Tier 1 (BM25):** Pure algorithmic — tokenize, compute IDF, score. No dependencies.
Port the `_bm25_scores()` function directly. ~60 lines of Rust.

**Tier 2 (LLM):** Send a prompt to the LLM client asking for a ranked JSON array of
source IDs. Parse response, reorder. Fallback to input order on error.

### 5.8 `llm/client.rs` — OpenAI-compatible chat client

**Python reference:** `client.py` (56 lines)

Single method: `chat(messages, temperature) -> Result<String>`.
POST to `{base_url}/chat/completions` with `reqwest`, parse response JSON.

```rust
pub struct LlmClient {
    config: LlmConfig,
    http: reqwest::Client,
}

impl LlmClient {
    pub async fn chat(
        &self,
        messages: &[Message],
        temperature: f32,
    ) -> Result<String> { ... }
}
```

### 5.9 `llm/expander.rs` — Query expansion

**Python reference:** `expander.py` (43 lines)

Send prompt to LLM, parse JSON array of strings, prepend original query.
Fallback to `[query]` on any error.

### 5.10 `llm/summarizer.rs` — Results summarization

**Python reference:** `summarizer.py` (49 lines)

Build context from sources, send prompt to LLM, return Markdown string.

### 5.11 `main.rs` — MCP server (webgate-mcp crate)

**Python reference:** `server.py` (246 lines)

Register three MCP tools:
- `webgate_onboarding()` — return operational guide JSON
- `webgate_fetch(url, max_chars?)` — single page fetch
- `webgate_query(queries, num_results_per_query?, lang?, backend?)` — full pipeline

Use `rmcp` (or the Rust MCP SDK when stable) for:
- Tool schema registration
- Stdio transport
- JSON-RPC handling

The binary name is `mcp-webgate` (configured in `Cargo.toml` `[[bin]]`).

---

## 6. Anti-flooding protections (MUST preserve)

These are the core value proposition. Every protection must exist in the Rust port:

| Protection | Python location | Rust location | Implementation |
|-----------|----------------|--------------|----------------|
| `max_download_mb` streaming cap | `fetcher.py:104-108` | `fetcher.rs` | `bytes_stream()` + byte counter + break |
| `max_result_length` per-page cap | `cleaner.py:163-190` | `cleaner.rs` | `apply_window()` line-boundary truncation |
| `max_query_budget` total budget | `query.py:187-197` | pipeline in lib.rs or query module | Budget ÷ candidates = per-page limit |
| `max_total_results` global cap | `query.py:99` | pipeline | `min(per_query × n_queries, cap)` |
| Binary extension filter | `url.py:61-64` | `url.rs` | Check before any network request |
| Streaming (no buffering) | `fetcher.py:88` | `fetcher.rs` | `reqwest` streaming — **never** `response.text()` |
| Unicode/BiDi sterilization | `cleaner.py:17-19` | `cleaner.rs` | Regex strip of control chars, ZWJ, BiDi overrides |

---

## 7. Build and distribution

### Development

```bash
# Linux/macOS — libxml2 is typically already installed
cargo run -p webgate-mcp -- --default-backend searxng

# Windows
vcpkg install libxml2:x64-windows
set VCPKG_ROOT=C:\vcpkg
cargo run -p webgate-mcp -- --default-backend searxng
```

### CI release builds (static linking)

Cross-compile for 5 targets with libxml2 statically linked:

| Target | OS | Libxml2 strategy |
|--------|-----|-----------------|
| `x86_64-unknown-linux-gnu` | ubuntu-latest | `apt install libxml2-dev` + static link flag |
| `aarch64-unknown-linux-gnu` | ubuntu-latest | `cross` with static libxml2 in Docker |
| `x86_64-apple-darwin` | macos-latest | `brew install libxml2` + static link |
| `aarch64-apple-darwin` | macos-latest | same (Apple Silicon native) |
| `x86_64-pc-windows-msvc` | windows-latest | `vcpkg install libxml2:x64-windows-static` |

Output: 5 self-contained binaries attached to GitHub Release. No runtime dependencies.

### Installation

```bash
# From crates.io
cargo install webgate-mcp

# From GitHub Release (prebuilt)
curl -L https://github.com/annibale-x/webgate/releases/latest/download/mcp-webgate-x86_64-unknown-linux-gnu -o mcp-webgate
chmod +x mcp-webgate

# Agent config (Claude Code, Gemini CLI, etc.)
{ "command": "mcp-webgate", "args": ["--config", "webgate.toml"] }
```

---

## 8. API contract (library crate)

The `webgate` crate exposes a high-level async API:

```rust
use webgate::{Config, FetchResult, QueryResult};

// Load config from file + env
let config = Config::load()?;

// Single page fetch
let result: FetchResult = webgate::fetch("https://example.com", &config).await?;
println!("{}", result.text);

// Full search pipeline
let result: QueryResult = webgate::query(
    &["rust async patterns", "tokio best practices"],
    &config,
).await?;
for source in &result.sources {
    println!("[{}] {} — {} chars", source.id, source.title, source.content.len());
}
```

Return types mirror the Python JSON output:

```rust
pub struct FetchResult {
    pub url: String,
    pub title: String,
    pub text: String,
    pub truncated: bool,
    pub char_count: usize,
}

pub struct QueryResult {
    pub queries: Vec<String>,
    pub sources: Vec<Source>,
    pub snippet_pool: Vec<SnippetEntry>,
    pub stats: Stats,
    pub summary: Option<String>,          // when LLM summarization is enabled
    pub llm_summary_error: Option<String>, // on LLM failure
}
```

---

## 9. Milestones

### M1 — Core library: fetch + clean (1 week)

- [ ] Workspace setup (`Cargo.toml`, two crates)
- [ ] `config.rs` — serde config with toml + env + clap
- [ ] `scraper/cleaner.rs` — libxml2 HTML cleaning + text sterilization pipeline
- [ ] `scraper/fetcher.rs` — reqwest concurrent fetcher with streaming cap, UA rotation, retry
- [ ] `utils/url.rs` — sanitize, dedup, binary filter, domain filter
- [ ] `lib.rs` — `webgate::fetch()` public API
- [ ] Tests: cleaner (port from Python test suite), fetcher (mock server)
- [ ] **Deliverable:** `webgate` crate compiles and passes tests

### M2 — MCP server with fetch tool (3 days)

- [ ] `main.rs` — MCP server with `webgate_fetch` tool via rmcp
- [ ] `webgate_onboarding` tool (static JSON)
- [ ] CLI argument parsing with clap
- [ ] Stdio transport working with Claude Code
- [ ] **Deliverable:** `cargo install webgate-mcp` provides working `mcp-webgate` binary with fetch

### M3 — Search backends + query pipeline (1 week)

- [ ] `backends/mod.rs` — `SearchBackend` trait
- [ ] `backends/searxng.rs` — first backend
- [ ] `backends/brave.rs`
- [ ] `backends/tavily.rs`
- [ ] `backends/exa.rs`
- [ ] `backends/serpapi.rs`
- [ ] `utils/reranker.rs` — BM25 deterministic reranking
- [ ] Full query pipeline: search → dedup → fetch → clean → rerank → assemble
- [ ] `webgate_query` MCP tool
- [ ] `webgate::query()` public library API
- [ ] Tests: backends (mock HTTP), full pipeline integration
- [ ] **Deliverable:** Feature parity with Python Phase 1–3 (no LLM)

### M4 — LLM features (1 week)

- [ ] `llm/client.rs` — OpenAI-compatible async chat client
- [ ] `llm/expander.rs` — query expansion
- [ ] `llm/summarizer.rs` — Markdown summary with citations
- [ ] LLM reranking in `reranker.rs`
- [ ] Adaptive budget redistribution
- [ ] Tests: LLM features with mock responses
- [ ] **Deliverable:** Full feature parity with Python Phase 4

### M5 — CI, release, publish (3 days)

- [ ] GitHub Actions `ci.yml` — test on ubuntu/windows/macos
- [ ] GitHub Actions `release.yml` — cross-compile 5 targets with static libxml2
- [ ] Publish `webgate` + `webgate-mcp` on crates.io
- [ ] README with installation instructions
- [ ] **Deliverable:** Prebuilt binaries on GitHub Releases, `cargo install webgate-mcp` works

### M6 — Zed extension (optional, 2 days)

- [ ] `integrations/zed/` — WASM extension that downloads the native binary
- [ ] No stub needed — binary is self-contained
- [ ] `extension.toml` with minimal settings (backend, config path)
- [ ] **Deliverable:** Zed extension in marketplace

---

## 10. Testing strategy

| Layer | Framework | What |
|-------|-----------|------|
| Unit: cleaner | `#[cfg(test)]` | Port existing Python test cases. Static HTML → expected clean text. |
| Unit: url utils | `#[cfg(test)]` | Sanitize, dedup, binary filter, domain filter |
| Unit: BM25 | `#[cfg(test)]` | Known documents → expected ranking order |
| Unit: config | `#[cfg(test)]` | TOML parsing, env override, CLI override |
| Integration: fetcher | `wiremock` | Mock HTTP server, test streaming cap, retry, UA rotation |
| Integration: backends | `wiremock` | Mock each backend's API, verify request format + response parsing |
| Integration: LLM | `wiremock` | Mock OpenAI endpoint, test expander/summarizer/reranker |
| Integration: pipeline | `wiremock` | Full query cycle with mocked backend + mocked pages |
| E2E: MCP | subprocess | Spawn `mcp-webgate`, send JSON-RPC via stdin, verify stdout |

---

## 11. Open questions

1. **MCP SDK choice:** `rmcp` is the most mature Rust MCP crate today. Monitor the official
   Anthropic Rust SDK — if it ships before M2, evaluate switching.

2. **libxml2 static linking on Windows:** `vcpkg` static triplet (`x64-windows-static-md`)
   works but requires testing in CI. Fallback: ship Windows binary with `libxml2.dll` sidecar.

3. **WASM target for `webgate` library:** If we want `wasm32-wasi` support (e.g., for
   serverless runtimes), libxml2 bindings won't compile to WASM. Would need a pure-Rust
   HTML parser fallback behind a feature flag. Out of scope for initial release.

4. **Feature flags:** Consider making LLM features optional via Cargo features
   (`webgate = { features = ["llm"] }`) to reduce binary size for users who don't need them.
