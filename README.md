# WebGate

[![Crates.io](https://img.shields.io/crates/v/webgate.svg)](https://crates.io/crates/webgate)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![MCP Protocol](https://img.shields.io/badge/MCP-Protocol-blueviolet)](https://spec.modelcontextprotocol.io/)
[![Latest Release](https://img.shields.io/badge/release-v0.1.12-purple.svg)](https://github.com/annibale-x/webgate/releases/tag/v0.1.12)
[![Alpha](https://img.shields.io/badge/status-alpha-orange.svg)](https://github.com/annibale-x/webgate/issues)

---

## What is WebGate

WebGate is a Rust library and MCP server that turns noisy web pages into
clean, right-sized text for LLM consumption.

Raw HTML is mostly junk: scripts, ads, navigation menus, cookie banners,
tracking pixels. Feeding it directly to an LLM floods the context window
with tens of thousands of useless tokens and leaves no room for reasoning.
WebGate strips all that noise, sterilizes the text, and enforces strict
size budgets so the model receives only the content that matters.

### What you get

Depending on the features you enable, WebGate can be three things:

| Use case | Crate | Feature flags | What it does |
|----------|-------|---------------|--------------|
| **HTML denoiser** | `webgate` | `default-features = false` | `clean()` — pure Rust HTML-to-text pipeline. Strips noise elements, sterilizes Unicode/BiDi, collapses whitespace. Zero network, zero config. Drop into any Rust project that processes web content for LLMs. |
| **Web content client** | `webgate` | `default` or `features = ["llm"]` | `fetch()` + `query()` — streaming HTTP fetcher with size caps, 8 search backends, BM25 reranking, optional LLM query expansion and summarization. Full pipeline from search query to structured results. |
| **MCP server** | `webgate-mcp` | all features | Native binary (`mcp-webgate`) that exposes `webgate_query`, `webgate_fetch`, and `webgate_onboarding` over MCP stdio. Single static binary, zero runtime dependencies. |

### When to use WebGate

- You're building an AI agent that needs web search and you want clean,
  budget-controlled text — not raw HTML.
- You're processing web pages in a Rust pipeline and need a reliable
  HTML-to-text cleaner that strips noise without losing real content.
- You want an MCP web search server that works as a single binary —
  no Python, no pip, no venv, no Docker (unless you want it).
- You need hard guarantees on output size: per-page caps, total budget
  caps, streaming download limits.

### When NOT to use WebGate

- You need a headless browser that renders JavaScript-heavy SPAs.
  WebGate parses static HTML — it doesn't execute JS.
- You need to preserve the visual layout or formatting of a page
  (tables, CSS grids, positioning). WebGate extracts text, not structure.
- You're building a web scraper that needs to navigate across pages,
  fill forms, or handle authentication flows.

---

## How it works

```
Question
  |
  +- (optional) LLM query expansion -> multiple search variants
  |
  +- Search via backend (SearXNG, Brave, Tavily, Exa, SerpAPI, Google, Bing, HTTP)
  |
  +- Deduplicate + filter binary URLs
  |
  +- Streaming fetch with per-page size cap
  |
  +- HTML cleaning -> plain text (noise elements, scripts, nav removed)
  |
  +- Unicode/BiDi sterilization
  |
  +- BM25 deterministic reranking
  |   +- (optional) LLM-assisted tier-2 reranking
  |
  +- Budget-aware truncation across all sources
  |
  +- (optional) LLM Markdown summary with inline citations
  |
  +- Structured JSON output
```

For a detailed explanation of each pipeline stage, BM25 parameters, adaptive budget allocation, and real compression metrics see [Under the Hood](docs/UNDER_THE_HOOD.md). For the full configuration reference (TOML, env vars, CLI args) see [Configuration](docs/CONFIGURATION.md). For ready-to-use examples see [Use Cases](docs/USE_CASES.md).

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

1. **CLI args** — `--default-backend`, `--brave-api-key`, etc.
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
adaptive_budget     = "auto"  # "auto" | "on" | "off" — budget allocation mode

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

For every setting with all three config methods (TOML, env vars, CLI args)
and plain-language descriptions, see the full [Configuration Reference](docs/CONFIGURATION.md).
Ready-to-use config examples are in [Use Cases](docs/USE_CASES.md) and [`examples/`](examples/).

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
| **SerpAPI** | API key | Multi-engine proxy (Google, Bing, DDG...). [serpapi.com](https://serpapi.com/) |
| **Google** | API key + CX | Custom Search. Free: 100 req/day. [programmablesearchengine.google.com](https://programmablesearchengine.google.com/) |
| **Bing** | API key | Web Search API. Free: 1,000 req/month. [Microsoft Azure](https://www.microsoft.com/en-us/bing/apis/bing-web-search-api) |
| **HTTP** | configurable | Generic REST backend — no code required, TOML-only config |

---

## LLM features (optional)

All opt-in — disabled by default, no data leaves your machine unless enabled.

| Feature | What it does |
|---------|-------------|
| **Query expansion** | Single query -> N complementary search variants |
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

<!-- RECENT_CHANGES_START -->
<!-- RECENT_CHANGES_END -->

## Alpha Status

WebGate is in **alpha**. Core functionality is stable and the server is used daily,
but the API surface may still change before 1.0.

**Feedback is very welcome.** If something doesn't work as expected, behaves oddly,
or you have a use case that isn't covered:

> [Open an issue on GitHub](https://github.com/annibale-x/webgate/issues)

Bug reports, configuration questions, and feature requests all help shape the roadmap.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines on:
- Development setup and workflow
- Code style and conventions
- Testing requirements
- Documentation standards
- Pull request process

## License

MIT License — see [LICENSE](LICENSE) for details.

## Links

- **[GitHub Repository](https://github.com/annibale-x/webgate)** — Source code and issues
<!-- - **[docs.rs](https://docs.rs/webgate)** — API documentation -->
<!-- - **[MCP Registry](https://registry.modelcontextprotocol.io/?q=webgate&all=1)** — WebGate on Model Context Protocol Registry -->
- **[MCP Protocol](https://modelcontextprotocol.io/specification/2025-11-25)** — Model Context Protocol specification

---

**Need help?** Check the [documentation](docs/) or open an [issue](https://github.com/annibale-x/webgate/issues) on GitHub.

<!-- mcp-name: io.github.annibale-x/mcp-webgate -->
