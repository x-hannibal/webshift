# webshift-mcp

[![Crates.io](https://img.shields.io/crates/v/webshift-mcp.svg)](https://crates.io/crates/webshift-mcp)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](https://github.com/x-monk/webshift/blob/main/LICENSE)
[![MCP Protocol](https://img.shields.io/badge/MCP-Protocol-blueviolet)](https://spec.modelcontextprotocol.io/)
[![Beta](https://img.shields.io/badge/status-beta-blue.svg)](https://github.com/x-monk/webshift/issues)

**Denoised web search MCP server — single static binary, zero runtime dependencies.**

`webshift-mcp` exposes three MCP tools over stdio:

| Tool | Description |
|------|-------------|
| `webshift_query` | Full search pipeline: search + fetch + clean + rerank + (optional) LLM summarize |
| `webshift_fetch` | Fetch and clean a single URL |
| `webshift_onboarding` | Returns a JSON guide for the agent (budgets, backends, tips) |

Built on top of the [`webshift`](https://crates.io/crates/webshift) library.

---

## Installation

```bash
cargo install webshift-mcp
```

The binary is called `mcp-webshift`.

---

## Quick start

### 1. Set up a search backend

The easiest option is [SearXNG](https://docs.searxng.org/) — free, self-hosted, no API key:

```bash
docker run -d -p 8080:8080 searxng/searxng
```

No Docker? Use a cloud backend — see [Search backends](#search-backends).

### 2. Configure your MCP client

```json
{
  "mcpServers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

That's it. Your AI agent now has web search with clean, budget-controlled text output.

For client-specific setup (Claude Desktop, Claude Code, Zed, Cursor, Windsurf, VS Code, Gemini CLI) see [docs/integrations/](https://github.com/x-monk/webshift/tree/main/docs/integrations).

---

## MCP tool parameters

### `webshift_query`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `queries` | string or list | required | Search query or list of queries |
| `num_results` | integer | 5 | Results per query |
| `lang` | string | none | Language filter (e.g. `"en"`) |
| `backend` | string | config default | Override search backend for this call |

### `webshift_fetch`

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | string | URL to fetch and clean |

---

## Configuration

Resolution order (highest priority first):

1. **CLI args** — `--default-backend`, `--brave-api-key`, etc.
2. **Environment variables** — `WEBSHIFT_*` prefix
3. **Config file** — `webshift.toml` (current dir, then `~/webshift.toml`)
4. **Built-in defaults**

### Config file (`webshift.toml`)

```toml
[server]
max_query_budget    = 32000   # total char budget across all sources
max_result_length   = 8000    # per-page char cap
max_total_results   = 20      # hard cap on results per call
max_download_mb     = 1       # streaming cap per page (MB)
search_timeout      = 8       # seconds
results_per_query   = 5
adaptive_budget     = "auto"  # "auto" | "on" | "off"

[backends]
default = "searxng"

[backends.searxng]
url = "http://localhost:8080"

[backends.brave]
api_key = "BSA-..."

[backends.tavily]
api_key = "tvly-..."

[backends.exa]
api_key = "..."

[backends.serpapi]
api_key = "..."
engine  = "google"

[backends.google]
api_key = "..."
cx      = "..."

[backends.bing]
api_key = "..."
market  = "en-US"

[llm]
enabled               = false
base_url              = "http://localhost:11434/v1"
model                 = "gemma3:27b"
expansion_enabled     = true
summarization_enabled = true
```

For the full configuration reference (all TOML keys, env vars, CLI args) see [docs/CONFIGURATION.md](https://github.com/x-monk/webshift/blob/main/docs/CONFIGURATION.md).

### Key CLI args

```bash
mcp-webshift \
  --default-backend searxng \
  --searxng-url http://localhost:8080 \
  --brave-api-key BSA-xxx \
  --google-api-key xxx --google-cx xxx \
  --bing-api-key xxx \
  --llm-enabled true \
  --llm-base-url http://localhost:11434/v1 \
  --llm-model gemma3:27b
```

### Key environment variables

```bash
WEBSHIFT_DEFAULT_BACKEND=searxng
WEBSHIFT_SEARXNG_URL=http://localhost:8080
WEBSHIFT_BRAVE_API_KEY=BSA-xxx
WEBSHIFT_LLM_ENABLED=true
WEBSHIFT_LLM_BASE_URL=http://localhost:11434/v1
WEBSHIFT_LLM_MODEL=gemma3:27b
```

---

## Search backends

| Backend | Auth | Notes |
|---------|------|-------|
| **SearXNG** | none | Self-hosted, free. Default: `http://localhost:8080` |
| **Brave** | API key | Free tier. [brave.com/search/api](https://brave.com/search/api/) |
| **Tavily** | API key | AI-oriented. [tavily.com](https://tavily.com/) |
| **Exa** | API key | Neural search. [exa.ai](https://exa.ai/) |
| **SerpAPI** | API key | Multi-engine proxy. [serpapi.com](https://serpapi.com/) |
| **Google** | API key + CX | Custom Search. Free: 100 req/day. [programmablesearchengine.google.com](https://programmablesearchengine.google.com/) |
| **Bing** | API key | Web Search API. Free: 1,000 req/month. [azure.microsoft.com](https://www.microsoft.com/en-us/bing/apis/bing-web-search-api) |
| **HTTP** | configurable | Generic REST backend — TOML-only config, no code required |

---

## LLM features (optional)

All opt-in — disabled by default. Works with any OpenAI-compatible API (OpenAI, Ollama, vLLM, LM Studio):

```toml
[llm]
enabled  = true
base_url = "http://localhost:11434/v1"
model    = "gemma3:27b"
```

| Feature | What it does |
|---------|-------------|
| Query expansion | Single query → N complementary search variants |
| Summarization | Markdown report with inline `[1]` `[2]` citations |
| LLM reranking | Tier-2 reranking on top of deterministic BM25 |

---

## Anti-flooding protections

Always active:

| Protection | Description |
|------------|-------------|
| `max_download_mb` | Streaming cap — never buffers full response |
| `max_result_length` | Hard cap on characters per cleaned page |
| `max_query_budget` | Total character budget across all sources |
| `max_total_results` | Hard cap on results per call |
| Binary filter | `.pdf`, `.zip`, `.exe`, etc. filtered before any network request |
| Unicode sterilization | BiDi control chars, zero-width chars removed |

---

## Links

- **[GitHub Repository](https://github.com/x-monk/webshift)** — Source code and issues
- **[webshift library](https://crates.io/crates/webshift)** — Rust library crate
- **[Configuration Reference](https://github.com/x-monk/webshift/blob/main/docs/CONFIGURATION.md)**
- **[Use Cases & Examples](https://github.com/x-monk/webshift/blob/main/docs/USE_CASES.md)**
- **[IDE & Agent Integration Guides](https://github.com/x-monk/webshift/tree/main/docs/integrations)**
- **[MCP Protocol](https://modelcontextprotocol.io/specification/2025-11-25)**

## License

MIT License — see [LICENSE](https://github.com/x-monk/webshift/blob/main/LICENSE) for details.
