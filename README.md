# webgate

[![Crates.io](https://img.shields.io/crates/v/webgate-mcp.svg)](https://crates.io/crates/webgate-mcp)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![MCP Protocol](https://img.shields.io/badge/MCP-Protocol-blueviolet)](https://spec.modelcontextprotocol.io/)

**Denoised web search library and MCP server** — native Rust port of [mcp-webgate](https://github.com/annibale-x/mcp-webgate).

Single static binary, zero runtime dependencies. Feeds clean, right-sized web content to LLM agents without flooding the context window.

---

## How it works

```
Question
  │
  ├─ (optional) LLM query expansion → multiple search variants
  │
  ├─ Search via backend (SearXNG, Brave, Tavily, Exa, SerpAPI, Google, Bing, HTTP)
  │
  ├─ Deduplicate + filter binary URLs
  │
  ├─ Streaming fetch with per-page size cap
  │
  ├─ HTML cleaning → plain text (noise elements, scripts, nav removed)
  │
  ├─ Unicode/BiDi sterilization
  │
  ├─ BM25 deterministic reranking
  │   └─ (optional) LLM-assisted tier-2 reranking
  │
  ├─ Budget-aware truncation across all sources
  │
  ├─ (optional) LLM Markdown summary with inline citations
  │
  └─ Structured JSON output
```

---

## Installation

### Binary (MCP server)

```bash
cargo install webgate-mcp
```

The binary is called `mcp-webgate`.

### From source

```bash
cargo install --path crates/webgate-mcp
```

### As a library

```toml
# Full pipeline (search + fetch + clean + rerank)
webgate = "0.1"

# Cleaner + fetcher only (no search backends)
webgate = { version = "0.1", default-features = false }

# Everything including LLM features
webgate = { version = "0.1", features = ["llm"] }
```

---

## Quick start

### 1. Set up a search backend

The easiest option is [SearXNG](https://docs.searxng.org/) — free, self-hosted, no API key:

```bash
docker run -d -p 4000:8080 searxng/searxng
```

No Docker? Use a cloud backend — see [Search backends](#search-backends).

### 2. Configure your MCP client

```json
{
  "mcpServers": {
    "webgate": {
      "command": "mcp-webgate",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

That's it. The agent now has `webgate_query`, `webgate_fetch`, and `webgate_onboarding`.

For client-specific setup see [docs/integrations/](docs/integrations/).

---

## MCP tools

| Tool | Description |
|------|-------------|
| `webgate_query` | Full search pipeline: search + fetch + clean + rerank + (optional) summarize |
| `webgate_fetch` | Single page fetch and clean |
| `webgate_onboarding` | Returns a JSON guide for the agent (budgets, backends, tips) |

### `webgate_query` parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `queries` | string or list | required | Search query or list of queries |
| `num_results` | integer | 5 | Results per query |
| `lang` | string | none | Language filter (e.g. `"en"`) |
| `backend` | string | config default | Override search backend |

---

## Configuration

Resolution order (highest priority first):

1. **CLI args** — `--default-backend`, `--debug`, etc.
2. **Environment variables** — `WEBGATE_*` prefix
3. **Config file** — `webgate.toml` (current dir, then `~/webgate.toml`)
4. **Built-in defaults**

### Config file

```toml
[server]
max_query_budget    = 32000   # total char budget across all sources
max_result_length   = 8000    # per-page char cap
max_total_results   = 20      # hard cap on results per call
max_download_mb     = 1       # streaming cap per page (MB)
search_timeout      = 8       # seconds
results_per_query   = 5
oversampling_factor = 2
adaptive_budget     = false   # [EXPERIMENTAL] proportional allocation

[backends]
default = "searxng"

[backends.searxng]
url = "http://localhost:4000"

[backends.brave]
api_key = "BSA-..."

[backends.tavily]
api_key = "tvly-..."

[backends.exa]
api_key = "..."

[backends.serpapi]
api_key = "..."
engine  = "google"    # google | bing | duckduckgo | yandex

[backends.google]
api_key = "..."
cx      = "..."       # Custom Search Engine ID

[backends.bing]
api_key = "..."
market  = "en-US"

[backends.http]
url           = "https://my-search.example.com/api/search"
query_param   = "q"
count_param   = "limit"
results_path  = "data.items"     # dot-path to results array in JSON response
title_field   = "title"
url_field     = "link"
snippet_field = "description"

[backends.http.headers]
"Authorization" = "Bearer my-token"

[llm]
enabled               = false
base_url              = "http://localhost:11434/v1"   # OpenAI-compatible
api_key               = ""
model                 = "gemma3:27b"
timeout               = 60
expansion_enabled     = true
summarization_enabled = true
llm_rerank_enabled    = false
```

Ready-to-use config examples are in [`examples/`](examples/).

### Key environment variables

```bash
WEBGATE_DEFAULT_BACKEND=searxng
WEBGATE_SEARXNG_URL=http://localhost:4000
WEBGATE_BRAVE_API_KEY=BSA-xxx
WEBGATE_GOOGLE_API_KEY=xxx
WEBGATE_GOOGLE_CX=xxx
WEBGATE_BING_API_KEY=xxx
WEBGATE_LLM_ENABLED=true
WEBGATE_LLM_BASE_URL=http://localhost:11434/v1
WEBGATE_LLM_MODEL=gemma3:27b
```

---

## Search backends

| Backend | Auth | Notes |
|---------|------|-------|
| **SearXNG** | none | Self-hosted, free. Default: `http://localhost:4000` |
| **Brave** | API key | Free tier. [brave.com/search/api](https://brave.com/search/api/) |
| **Tavily** | API key | AI-oriented. [tavily.com](https://tavily.com/) |
| **Exa** | API key | Neural search. [exa.ai](https://exa.ai/) |
| **SerpAPI** | API key | Multi-engine proxy (Google, Bing, DDG…). [serpapi.com](https://serpapi.com/) |
| **Google** | API key + CX | Custom Search. Free: 100 req/day. [programmablesearchengine.google.com](https://programmablesearchengine.google.com/) |
| **Bing** | API key | Web Search API. Free: 1,000 req/month. [Microsoft Azure](https://www.microsoft.com/en-us/bing/apis/bing-web-search-api) |
| **HTTP** | configurable | Generic REST backend — no code required, TOML-only config |

---

## LLM features (optional)

All opt-in — disabled by default, no data leaves your machine unless enabled.

| Feature | What it does |
|---------|-------------|
| **Query expansion** | Single query → N complementary search variants |
| **Summarization** | Markdown report with inline `[1]` `[2]` citations |
| **LLM reranking** | Tier-2 reranking on top of deterministic BM25 |

> **Cross-language normalization (bonus):** when BM25 reranking surfaces pages in
> foreign languages (e.g. Chinese, Japanese, Arabic), the LLM summarizer still
> produces the final report in the prompt language. The agent receives clean,
> readable output regardless of the language mix in the source pages.

Works with any OpenAI-compatible API (OpenAI, Ollama, vLLM, LM Studio, etc.):

```toml
[llm]
enabled  = true
base_url = "http://localhost:11434/v1"
model    = "gemma3:27b"
```

---

## Anti-flooding protections

Always active — the core value proposition:

| Protection | Description |
|------------|-------------|
| `max_download_mb` | Streaming cap — never buffers full response |
| `max_result_length` | Hard cap on characters per cleaned page |
| `max_query_budget` | Total character budget across all sources |
| `max_total_results` | Hard cap on results per call |
| Binary filter | `.pdf`, `.zip`, `.exe`, etc. filtered **before** any network request |
| Unicode sterilization | BiDi control chars, zero-width chars removed |

---

## Library usage

```rust
use webgate::{Config, clean, fetch, query};

// Clean raw HTML
let result = clean("<html><body><p>Hello world</p></body></html>", 8000);

// Fetch and clean a single page
let config = Config::default();
let page = fetch("https://example.com", &config).await?;

// Full search pipeline
let results = query(&["rust async programming"], &config).await?;
for source in &results.sources {
    println!("[{}] {} — {} chars", source.id, source.title, source.content.len());
}
```

### Feature flags

| Feature | Default | Enables |
|---------|---------|---------|
| `backends` | on | All search backends + query pipeline |
| `llm` | off | LLM client, expander, summarizer, LLM reranking |

---

## Integrations

| Platform | Guide |
|----------|-------|
| Claude Desktop, Claude Code, Zed, Cursor, Windsurf, VS Code | [IDE Integration](docs/integrations/IDE.md) |
| Gemini CLI, Claude CLI, custom agents | [Agent Integration](docs/integrations/AGENT.md) |

---

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide.

```bash
cargo build                              # build all crates
cargo test                               # unit tests (mocked, no services needed)
cargo test -- --ignored                  # integration tests (requires test.toml)
cargo run -p robot -- harness "query"   # diagnostic harness with stats
```

---

## License

MIT
