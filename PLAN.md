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
- **Embedding:** Agents written in Rust can call `webgate::fetch()`, `webgate::query()`
  and `webgate::clean()` in-process, without spawning a subprocess or speaking JSON-RPC.
- **Containers:** A scratch Docker image with one binary (~8 MB) replaces ~200 MB Python images.
- **CI/Edge:** No interpreter setup step. Copy binary, run.
- **Composability:** Other Rust crates can depend on `webgate` and build on top of it
  (aggregators, proxy MCP servers, agent toolkits, RAG pipelines).
- **HTML cleaning for LLM pipelines:** `webgate::clean()` is a first-class public API,
  usable standalone to strip noise elements and sterilize HTML into LLM-ready text.
  Removing excess markup can reduce token usage by ~70% while preserving all meaningful
  content — useful for any Rust project that processes web content with an LLM.

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
│   │       ├── lib.rs              # public API: fetch(), query(), clean(), Config
│   │       ├── config.rs           # serde config (toml + env + CLI)
│   │       ├── scraper/
│   │       │   ├── mod.rs
│   │       │   ├── fetcher.rs      # reqwest concurrent fetcher, streaming, UA rotation
│   │       │   └── cleaner.rs      # scraper/html5ever HTML cleaning + regex text sterilization
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
│   ├── webgate-mcp/                # binary crate (crates.io: webgate-mcp)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs             # MCP server: tool registration, stdio transport
│   │                               # [[bin]] name = "mcp-webgate"
│   │
│   └── robot/                      # internal dev tool (publish = false)
│       ├── Cargo.toml
│       └── src/
│           └── main.rs             # bump, promote, unpromote, publish
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
| `lxml` | **`scraper`** (0.20+) | HTML parsing + CSS selectors | Pure Rust (html5ever). See §4. |
| `httpx` | **`reqwest`** + **`tokio`** | Async HTTP, streaming, connection pool | `reqwest` stream API mirrors httpx `client.stream()` pattern |
| `pydantic` + `tomllib` | **`serde`** + **`toml`** | Config deserialization | Derive-based, zero boilerplate |
| `re` | **`regex`** | Unicode/BiDi sterilization, noise line filter | Same patterns, better performance |
| — | **`rand`** | UA rotation | `rand::seq::SliceRandom::choose()` |
| `urllib.parse` | **`url`** | URL parsing, sanitization | `url::Url` for parse/rebuild |
| — | **`serde_json`** | JSON serialization of tool output | |
| — | **`thiserror`** | Error types | |
| — | **`tracing`** | Structured logging (debug/trace) | |

> **No C dependencies.** The library is pure Rust + pure Rust transitive deps.
> This enables static binaries on all targets without system package requirements,
> and keeps the door open for a future `wasm32-wasi` feature-flagged build.

### `webgate-mcp` (binary)

| Dependency | Purpose |
|-----------|---------|
| `webgate` | The library (all features enabled) |
| **`rmcp`** | MCP server, stdio transport, tool registration (official Anthropic Rust SDK) |
| **`clap`** | CLI argument parsing |
| **`tokio`** (full) | Async runtime |

### Feature flags

The `webgate` library uses Cargo feature flags to keep the default footprint minimal:

| Feature | Default | Enables |
|---------|---------|---------|
| `llm` | off | `llm/` module: query expansion, summarization, LLM reranking |
| `backends` | on | All search backends (searxng, brave, tavily, exa, serpapi) |

Users who want only the cleaner or the fetcher add:

```toml
webgate = { version = "0.1", default-features = false }
```

`webgate-mcp` enables all features.

---

## 4. HTML cleaning: pure-Rust approach

### Why not libxml2

The original plan proposed `libxml` Rust bindings (wrapping libxml2 in C) to reuse the
same XPath expressions from the Python version. After analysis, this was rejected:

| Concern | Detail |
|---------|--------|
| C dependency | Breaks the "zero system deps" goal. Requires `apt install libxml2-dev` / `brew install libxml2` / `vcpkg` on Windows. Static linking adds CI complexity, especially on Windows (~1–2 extra days). |
| WASM | `libxml2` bindings cannot compile to `wasm32-wasi`. Pure Rust parsers can, behind a feature flag. |
| Overkill | The XPath used is a single fixed noise-removal pattern. No dynamic queries, no XPath axes, no namespace handling. CSS selectors cover this use case completely. |

### Chosen approach: `scraper` (html5ever)

The `scraper` crate (pure Rust, based on html5ever — Mozilla's production HTML parser)
covers everything the cleaner needs with a single CSS selector string:

```rust
use scraper::{Html, Selector};

static NOISE_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse(
        "script,style,nav,footer,header,aside,form,\
         iframe,noscript,svg,button,input,select,textarea"
    ).unwrap()
});

pub fn clean_html(raw: &str) -> String {
    let mut doc = Html::parse_document(raw);
    let to_remove: Vec<_> = doc.select(&NOISE_SEL)
        .map(|el| el.id())
        .collect();
    for id in to_remove {
        doc.tree.get_mut(id).unwrap().detach();
    }
    doc.root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
}
```

This is semantically equivalent to the Python `lxml` XPath approach. The element set
being removed is identical; only the query language changes (CSS selectors vs XPath).

Alternatives considered and rejected:

| Crate | Why not |
|-------|---------|
| `libxml` | C bindings to libxml2 — breaks zero-deps goal, blocks WASM, complex Windows CI. See above. |
| `lol_html` | Streaming rewriter (Cloudflare) — great for transforms, poor for "parse → query → extract text" pattern. |
| `html5ever` | Low-level tokenizer/tree builder. No query layer — `scraper` wraps it at the right abstraction. |
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
    pub max_download_mb: u32,              // default 1
    pub max_result_length: usize,          // default 8000
    pub search_timeout: u64,               // default 8
    pub oversampling_factor: u32,          // default 2
    pub auto_recovery_fetch: bool,         // default false
    pub max_total_results: usize,          // default 20
    pub max_query_budget: usize,           // default 32000
    pub max_search_queries: usize,         // default 5
    pub results_per_query: usize,          // default 5
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
- Parse HTML with `Html::parse_document()`
- Remove noise nodes via CSS selector (see §4)
- Collect text via `.text()` iterator

**Stage 2: `clean_text(text) -> String`**
- Unicode/BiDi sterilization regex (same `_UNICODE_JUNK` pattern)
- Typography normalization table (smart quotes → ASCII, dashes, ligatures)
- Whitespace collapse
- Line-by-line noise filtering (`_NOISE_LINE` regex)
- Short-line / date-only filtering
- Duplicate line removal
- Collapse 3+ newlines to double

**Additional functions:**
- `extract_title(raw) -> String` — CSS selector `title`
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
Only available when the `llm` feature flag is enabled.

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

Use `rmcp` (official Anthropic Rust MCP SDK) for:
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
| `max_query_budget` total budget | `query.py:187-197` | pipeline in `lib.rs` | Budget ÷ candidates = per-page limit |
| `max_total_results` global cap | `query.py:99` | pipeline | `min(per_query × n_queries, cap)` |
| Binary extension filter | `url.py:61-64` | `url.rs` | Check before any network request |
| Streaming (no buffering) | `fetcher.py:88` | `fetcher.rs` | `reqwest` streaming — **never** `response.text()` |
| Unicode/BiDi sterilization | `cleaner.py:17-19` | `cleaner.rs` | Regex strip of control chars, ZWJ, BiDi overrides |

---

## 7. Build and distribution

### Development

```bash
# Linux/macOS/Windows — no system packages required (pure Rust)
cargo run -p webgate-mcp -- --default-backend searxng
```

### CI release builds (static linking)

Cross-compile for 5 targets. All pure Rust — no C library complications:

| Target | OS | Strategy |
|--------|-----|---------|
| `x86_64-unknown-linux-gnu` | ubuntu-latest | standard `cargo build --release` |
| `aarch64-unknown-linux-gnu` | ubuntu-latest | `cross` |
| `x86_64-apple-darwin` | macos-latest | standard |
| `aarch64-apple-darwin` | macos-latest | standard (Apple Silicon native) |
| `x86_64-pc-windows-msvc` | windows-latest | standard — no vcpkg required |

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
use webgate::{Config, FetchResult, QueryResult, CleanResult};

// Load config from file + env
let config = Config::load()?;

// Standalone HTML cleaning (no feature flags required)
let result: CleanResult = webgate::clean(raw_html, 8000);
println!("{}", result.text); // LLM-ready plain text

// Single page fetch + clean
let result: FetchResult = webgate::fetch("https://example.com", &config).await?;
println!("{}", result.text);

// Full search pipeline (requires `backends` feature)
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
pub struct CleanResult {
    pub text: String,
    pub truncated: bool,
    pub char_count: usize,
}

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
    pub summary: Option<String>,           // when `llm` feature + summarization enabled
    pub llm_summary_error: Option<String>, // on LLM failure
}
```

---

## 9. Milestones

### M1 — Core library: fetch + clean (1 week)

- [x] Workspace setup: add `robot` crate, feature flags skeleton, shared version
- [x] `config.rs` — serde config with toml + env + clap
- [x] `scraper/cleaner.rs` — html5ever/scraper HTML cleaning + text sterilization pipeline
- [x] `scraper/fetcher.rs` — reqwest concurrent fetcher with streaming cap, UA rotation, retry
- [x] `utils/url.rs` — sanitize, dedup, binary filter, domain filter
- [x] `lib.rs` — `webgate::clean()` and `webgate::fetch()` public API
- [x] `robot` — `bump`, `test`, `promote`, `unpromote`, `publish` commands
- [x] Tests: cleaner (port from Python test suite), fetcher (mock server)
- [x] **Deliverable:** `webgate` crate compiles and passes tests; `robot` operational

### M2 — MCP server with fetch tool (3 days)

- [x] `main.rs` — MCP server with `webgate_fetch` tool via `rmcp`
- [x] `webgate_onboarding` tool (static JSON)
- [x] CLI argument parsing with clap
- [x] Stdio transport working with Claude Code
- [x] **Deliverable:** `cargo install webgate-mcp` provides working `mcp-webgate` binary with fetch

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
- [ ] LLM reranking in `reranker.rs` (behind `llm` feature flag)
- [ ] Adaptive budget redistribution
- [ ] Tests: LLM features with mock responses
- [ ] **Deliverable:** Full feature parity with Python Phase 4

### M5 — CI, release, publish (3 days)

- [ ] GitHub Actions `ci.yml` — test on ubuntu/windows/macos
- [ ] GitHub Actions `release.yml` — cross-compile 5 targets (pure Rust, no C deps)
- [ ] Publish `webgate` + `webgate-mcp` on crates.io
- [ ] README with installation instructions + standalone cleaner usage examples
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

## 11. `robot` — internal dev tool

A small Rust binary in `crates/robot/` (workspace member, `publish = false`) that automates
the release workflow. Built with `clap`.

### Commands

#### `robot bump [X.Y.Z]`

Updates the workspace version and commits.

1. Read current version from `[workspace.package] version` in root `Cargo.toml`.
2. If `X.Y.Z` is provided, use it; otherwise increment the patch component (`Z+1`).
   Starting version: `0.0.1`.
3. Write the new version to root `Cargo.toml`.
4. `git add Cargo.toml Cargo.lock CHANGELOG.md`
5. `git commit -m "chore(release): bump to X.Y.Z"`

> Claude always updates `CHANGELOG.md` before running `robot bump`.

#### `robot promote`

Validates, merges to `main`, tags, and returns to `dev`.

1. `cargo build --release` — abort on failure.
2. `cargo test` — abort on failure.
3. Read version from workspace `Cargo.toml`.
4. `git checkout main && git merge dev --no-ff -m "release: vX.Y.Z"`
5. `git tag vX.Y.Z`
6. `git push origin main --tags`
7. `git checkout dev`

#### `robot unpromote`

Undoes the last promote (use immediately after a bad promote).

1. Read the last tag from `git describe --tags --abbrev=0`.
2. `git push origin :refs/tags/vX.Y.Z` — delete remote tag.
3. `git tag -d vX.Y.Z` — delete local tag.
4. `git checkout main && git reset --hard HEAD~1`
5. `git push origin main --force-with-lease`
6. `git checkout dev`

#### `robot publish`

Publishes both crates to crates.io. Use after `promote`, starting from M5.

1. `cargo publish -p webgate`
2. Wait for crates.io index propagation (~15 s).
3. `cargo publish -p webgate-mcp`

### Versioning

All crates share a single version via `[workspace.package] version`. Individual crates
declare `version.workspace = true`. A single `robot bump` call is sufficient.

---

## 12. Open questions

1. **MCP SDK:** `rmcp` is the official Anthropic Rust SDK as of 2025. Use it from M2 onward.
   Monitor for breaking changes on the 0.x series.

2. **WASM target for `webgate` library:** With the switch to pure-Rust `scraper`/html5ever,
   a `wasm32-wasi` build is now architecturally possible. Gated behind a feature flag
   (`wasm`) and out of scope for initial release, but no longer blocked by C deps.

3. **Feature flags:** `llm` feature is optional (see §3). Consider also making individual
   backends opt-in for users who only need one search provider.

4. **`webgate::clean()` as a standalone use case:** The cleaner is exposed as a first-class
   public API (not just an internal step). This opens a secondary audience: any Rust project
   doing HTML → LLM text conversion, independently of the MCP or search features.
