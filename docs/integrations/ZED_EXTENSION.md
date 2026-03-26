# Zed Extension

The `mcp-webshift` Zed extension installs and manages the native binary automatically.
No manual PATH setup, no config file editing — everything is handled through Zed's
**Configure Server** modal.

---

## Installation

1. Open Zed → **Extensions** (`Cmd+Shift+X` / `Ctrl+Shift+X`)
2. Search for **mcp-webshift**
3. Click **Install**

The first time the context server starts, the extension downloads the correct
native binary for your platform from GitHub Releases. No runtime required.

---

## Configuration

Right-click **mcp-webshift** in the **Context Servers** panel → **Configure Server**.

All settings are optional. The server starts with sensible defaults if you leave
everything blank. At minimum, set **default_backend** and the corresponding
credential (API key or URL).

### Quick examples

**SearXNG (self-hosted, no API key)**

| Setting | Value |
|---------|-------|
| `default_backend` | `searxng` |
| `searxng_url` | `http://localhost:8080` |

**Brave**

| Setting | Value |
|---------|-------|
| `default_backend` | `brave` |
| `brave_api_key` | `BSA-...` |

**With LLM summarization (Ollama)**

| Setting | Value |
|---------|-------|
| `default_backend` | `searxng` |
| `llm_enabled` | `true` |
| `llm_base_url` | `http://localhost:11434/v1` |
| `llm_model` | `gemma3:27b` |

---

## Full settings reference

### Search & fetch

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `default_backend` | string | `searxng` | Active backend: `searxng` \| `brave` \| `tavily` \| `exa` \| `serpapi` \| `google` \| `bing` \| `http` |
| `language` | string | `en` | Language tag passed to backends, e.g. `en`, `it`, `all` |
| `search_timeout` | integer | `8` | Per-request timeout in seconds |
| `results_per_query` | integer | `5` | Results requested per backend query |
| `max_total_results` | integer | `20` | Hard cap on results returned per call |
| `oversampling_factor` | integer | `2` | Fetch `results_per_query × factor` candidates before filtering |
| `auto_recovery_fetch` | boolean | `false` | Fill failed fetches from the reserve pool |
| `max_download_mb` | integer | `1` | Streaming cap per page in MB |
| `max_result_length` | integer | `8000` | Hard character cap per cleaned page |
| `max_query_budget` | integer | `32000` | Total character budget across all sources per call |
| `max_search_queries` | integer | `5` | Max queries per call, including LLM-expanded variants |
| `adaptive_budget` | string | `auto` | Budget allocation after BM25: `auto` \| `on` \| `off` |
| `adaptive_budget_fetch_factor` | integer | `3` | Fetch `max_result_length × factor` chars before trimming |
| `blocked_domains` | string | | Comma-separated blocklist, e.g. `spam.com,ads.net` |
| `allowed_domains` | string | | Comma-separated allowlist — only these domains pass |

### Backend credentials

| Setting | Type | Description |
|---------|------|-------------|
| `searxng_url` | string | SearXNG instance URL (default: `http://localhost:8080`) |
| `brave_api_key` | string | Brave Search API key |
| `tavily_api_key` | string | Tavily API key |
| `exa_api_key` | string | Exa API key |
| `serpapi_api_key` | string | SerpAPI key |
| `google_api_key` | string | Google Custom Search API key |
| `google_cx` | string | Google Custom Search Engine ID |
| `bing_api_key` | string | Bing Web Search API key |
| `bing_market` | string | Bing market code, e.g. `en-US`, `it-IT` |

### LLM features (optional)

All disabled by default. Work with any OpenAI-compatible API.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `llm_enabled` | boolean | `false` | Enable LLM features |
| `llm_base_url` | string | `http://localhost:11434/v1` | OpenAI-compatible API base URL |
| `llm_api_key` | string | | API key (leave empty for Ollama and other local servers) |
| `llm_model` | string | `gemma3:27b` | Model name |
| `llm_timeout` | integer | `30` | LLM request timeout in seconds |
| `llm_expansion_enabled` | boolean | `false` | Expand single queries into complementary variants |
| `llm_summarization_enabled` | boolean | `false` | Add a Markdown summary with citations to query output |
| `llm_rerank_enabled` | boolean | `false` | LLM-assisted tier-2 reranking on top of BM25 |
| `llm_max_summary_words` | integer | `0` | Max words in summary (0 = auto-derived from budget) |
| `llm_input_budget_factor` | integer | `3` | LLM input budget multiplier: `max_query_budget × factor` |

---

## How settings are applied

The extension translates each non-empty setting directly into a CLI argument
when launching the binary. For example:

```
default_backend = "brave"   →   --default-backend brave
max_result_length = 12000   →   --max-result-length 12000
llm_enabled = true          →   --llm-enabled true
```

This means each Zed instance gets its own independent configuration — no
conflict with other clients running the same binary with different settings.

Precedence inside the binary: `CLI args (extension) > env vars > webshift.toml > defaults`.
If you have a `webshift.toml` in your home directory, extension settings override it.

---

## Alternative: manual setup (without the extension)

If you prefer not to use the extension, add the server directly to
`~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "mcp-webshift": {
      "command": {
        "path": "mcp-webshift",
        "args": ["--default-backend", "searxng"]
      }
    }
  }
}
```

The binary must be on your PATH (`cargo install webshift-mcp`).

---

**See also**: [IDE Integration](./IDE.md) · [Configuration reference](../../README.md#configuration) · [Backends](../../README.md#search-backends)
