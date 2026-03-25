# Configuration Reference

webgate can be configured in three ways. When the same setting is specified
in more than one place, the highest-priority source wins:

```
CLI argument  >  environment variable  >  webgate.toml file  >  built-in default
```

**Example:** if `webgate.toml` says `max_total_results = 20` but you set
`WEBGATE_MAX_TOTAL_RESULTS=5` in your shell, the value used will be **5**.

---

## Where to put the config file

webgate looks for `webgate.toml` in two places, in this order:

1. The current working directory (where you run the command)
2. Your home directory (`~/webgate.toml`)

If no file is found, built-in defaults are used — everything works out of the box
with SearXNG on `localhost:8080`.

---

## Quick example

A minimal `webgate.toml` that uses a local SearXNG and an Ollama LLM:

```toml
[backends]
default = "searxng"

[backends.searxng]
url = "http://localhost:4000"

[llm]
enabled  = true
base_url = "http://localhost:11434/v1"
model    = "gemma3:27b"
```

Everything else uses sensible defaults. See [USE_CASES.md](USE_CASES.md) for
more complete examples.

---

## All settings

### Server settings

These control how webgate searches, fetches, and processes web pages.
They are the "safety valves" that prevent the LLM context window from being
flooded with too much text.

| TOML key | Env var | CLI arg | Default | What it does |
|----------|---------|---------|---------|--------------|
| `server.max_total_results` | `WEBGATE_MAX_TOTAL_RESULTS` | — | `20` | **Maximum number of web pages** returned per search. If you set this to 5, webgate will return at most 5 pages of content, even if the search engine found 100 results. |
| `server.max_query_budget` | `WEBGATE_MAX_QUERY_BUDGET` | — | `32000` | **Total character budget** shared across all pages. Think of it as a "text piggy bank": all pages together cannot exceed this number of characters. If you have 5 pages and a budget of 16,000, each page gets up to 3,200 characters. |
| `server.max_result_length` | `WEBGATE_MAX_RESULT_LENGTH` | — | `8000` | **Per-page character cap.** No single page can exceed this, even if the budget allows more. This prevents one very long page from eating all the budget. |
| `server.max_download_mb` | `WEBGATE_MAX_DOWNLOAD_MB` | — | `1` | **Download size limit** in megabytes. webgate streams each page and stops downloading once this limit is reached. This protects against accidentally downloading a 500 MB file. |
| `server.search_timeout` | `WEBGATE_SEARCH_TIMEOUT` | — | `8` | **Timeout in seconds** for each HTTP request (both search API calls and page fetches). Slow pages are abandoned after this time. |
| `server.results_per_query` | `WEBGATE_RESULTS_PER_QUERY` | — | `5` | How many results to request **per search query**. With LLM query expansion, one question might become 5 queries, each requesting 5 results = 25 candidates before dedup. |
| `server.max_search_queries` | `WEBGATE_MAX_SEARCH_QUERIES` | — | `5` | Maximum number of search queries allowed. Limits how many parallel searches the LLM expander can generate. |
| `server.oversampling_factor` | `WEBGATE_OVERSAMPLING_FACTOR` | — | `2` | **Oversampling multiplier.** Each query asks the search engine for `results_per_query × oversampling_factor` results. The extra results become the "reserve pool" — used to replace failed fetches and shown as snippet-only references. Value of 2 means: ask for twice as many results as needed. |
| `server.language` | — | — | `"en"` | **Language hint** sent to the search backend (BCP-47 code like `"en"`, `"it"`, `"de"`). Helps the search engine return results in your preferred language. Set to `""` (empty) to let the backend decide. |
| `server.adaptive_budget` | `WEBGATE_ADAPTIVE_BUDGET` | — | `"auto"` | **Budget allocation mode.** Controls how the character budget is divided among pages. See [Adaptive budget](#adaptive-budget) below. Values: `"auto"`, `"on"`/`true`, `"off"`/`false`. |
| `server.adaptive_budget_fetch_factor` | `WEBGATE_ADAPTIVE_BUDGET_FETCH_FACTOR` | — | `3` | When adaptive budget is active, webgate downloads more text per page initially (up to `max_result_length × this factor`) so it has enough material to redistribute later. |
| `server.auto_recovery_fetch` | `WEBGATE_AUTO_RECOVERY_FETCH` | — | `false` | **Gap-fill mode.** When enabled, if some pages fail to load (timeout, 404, bot-block), webgate automatically tries backup URLs from the reserve pool. |
| `server.blocked_domains` | — | — | `[]` | List of domains to **never** fetch (e.g. `["pinterest.com", "quora.com"]`). |
| `server.allowed_domains` | — | — | `[]` | If non-empty, **only** these domains will be fetched. Everything else is blocked. |
| `server.debug` | `WEBGATE_DEBUG` | `--debug` | `false` | Enable debug logging. |
| `server.log_file` | `WEBGATE_LOG_FILE` | `--log-file` | `""` | Write logs to this file (in addition to stderr). |
| `server.trace` | `WEBGATE_TRACE` | `--trace` | `false` | Enable trace-level logging (very verbose). |

### Adaptive budget

By default (`"auto"`), webgate decides automatically whether to redistribute
the character budget based on how different the BM25 relevance scores are:

- **If scores are spread out** (one page is much more relevant than others),
  the top-ranked page gets a bigger share of the budget, and low-ranked pages
  get less. This means you get more text from the best sources.
- **If scores are similar** (all pages are roughly equally relevant),
  every page gets the same share. Redistributing would make no real difference.

The decision is based on the "dominance ratio": how much more budget the
top page would get compared to a flat split. If it would get 50% more or higher,
adaptive mode kicks in.

You can also force it:
- `adaptive_budget = "on"` or `adaptive_budget = true` — always redistribute
- `adaptive_budget = "off"` or `adaptive_budget = false` — always use equal shares

---

### Backend settings

These tell webgate which search engine to use and how to connect to it.
You only need to configure the backend(s) you actually want to use.

| TOML key | Env var | CLI arg | Default | What it does |
|----------|---------|---------|---------|--------------|
| `backends.default` | `WEBGATE_DEFAULT_BACKEND` | `--default-backend` | `"searxng"` | Which search backend to use. One of: `searxng`, `brave`, `tavily`, `exa`, `serpapi`, `google`, `bing`, `http`. |

#### SearXNG (self-hosted, free, no API key)

| TOML key | Env var | Default | What it does |
|----------|---------|---------|--------------|
| `backends.searxng.url` | `WEBGATE_SEARXNG_URL` | `"http://localhost:8080"` | URL of your SearXNG instance. |

```toml
[backends.searxng]
url = "http://localhost:4000"
```

#### Brave Search

| TOML key | Env var | Default | What it does |
|----------|---------|---------|--------------|
| `backends.brave.api_key` | `WEBGATE_BRAVE_API_KEY` | `""` | Your Brave Search API key. Get one free at [brave.com/search/api](https://brave.com/search/api/). |
| `backends.brave.safesearch` | — | `1` | Safe search level: `0` = off, `1` = moderate, `2` = strict. |

```toml
[backends.brave]
api_key = "BSA-xxxxxxxxxxxx"
```

#### Tavily

| TOML key | Env var | Default | What it does |
|----------|---------|---------|--------------|
| `backends.tavily.api_key` | `WEBGATE_TAVILY_API_KEY` | `""` | Your Tavily API key. Get one at [tavily.com](https://tavily.com/). |
| `backends.tavily.search_depth` | — | `"basic"` | Search depth: `"basic"` (faster) or `"advanced"` (more thorough). |

```toml
[backends.tavily]
api_key = "tvly-xxxxxxxxxxxx"
search_depth = "basic"
```

#### Exa

| TOML key | Env var | Default | What it does |
|----------|---------|---------|--------------|
| `backends.exa.api_key` | `WEBGATE_EXA_API_KEY` | `""` | Your Exa API key. Get one at [exa.ai](https://exa.ai/). |
| `backends.exa.num_sentences` | — | `3` | Number of snippet sentences returned per result. |
| `backends.exa.search_type` | — | `"neural"` | Search type: `"neural"`, `"keyword"`, or `"auto"`. |

```toml
[backends.exa]
api_key = "exa-xxxxxxxxxxxx"
```

#### SerpAPI

| TOML key | Env var | Default | What it does |
|----------|---------|---------|--------------|
| `backends.serpapi.api_key` | `WEBGATE_SERPAPI_API_KEY` | `""` | Your SerpAPI key. Get one at [serpapi.com](https://serpapi.com/). |
| `backends.serpapi.engine` | `WEBGATE_SERPAPI_ENGINE` | `"google"` | Which engine SerpAPI should use: `"google"`, `"bing"`, `"duckduckgo"`, `"yandex"`, etc. |
| `backends.serpapi.gl` | `WEBGATE_SERPAPI_GL` | `"us"` | Country code for results (ISO 3166-1). |
| `backends.serpapi.hl` | `WEBGATE_SERPAPI_HL` | `"en"` | Language code for results. |
| `backends.serpapi.safe` | — | `"off"` | Safe search: `"off"`, `"active"`. |

```toml
[backends.serpapi]
api_key = "xxxxxxxxxxxx"
engine  = "google"
```

#### Google Custom Search

| TOML key | Env var | CLI arg | Default | What it does |
|----------|---------|---------|---------|--------------|
| `backends.google.api_key` | `WEBGATE_GOOGLE_API_KEY` | `--google-api-key` | `""` | Your Google API key. |
| `backends.google.cx` | `WEBGATE_GOOGLE_CX` | `--google-cx` | `""` | Your Custom Search Engine ID. Create one at [programmablesearchengine.google.com](https://programmablesearchengine.google.com/). |

Free tier: 100 requests/day.

```toml
[backends.google]
api_key = "AIza..."
cx      = "a1b2c3..."
```

#### Bing Web Search

| TOML key | Env var | CLI arg | Default | What it does |
|----------|---------|---------|---------|--------------|
| `backends.bing.api_key` | `WEBGATE_BING_API_KEY` | `--bing-api-key` | `""` | Your Bing API key (Azure Cognitive Services). |
| `backends.bing.market` | `WEBGATE_BING_MARKET` | `--bing-market` | `"en-US"` | Market code for result localization (e.g. `"en-US"`, `"it-IT"`, `"de-DE"`). |

Free tier: 1,000 requests/month.

```toml
[backends.bing]
api_key = "xxxxxxxxxxxx"
market  = "en-US"
```

#### HTTP (generic REST backend)

Connect to **any** REST search API without writing code. Just describe the
API shape in TOML:

| TOML key | Env var | Default | What it does |
|----------|---------|---------|--------------|
| `backends.http.url` | — | `""` | Base URL of the search endpoint (e.g. `https://api.example.com/search`). |
| `backends.http.method` | — | `"GET"` | HTTP method: `"GET"` or `"POST"`. |
| `backends.http.query_param` | — | `"q"` | Name of the query string parameter for the search text. |
| `backends.http.count_param` | — | `"count"` | Name of the parameter for the result count. Set to `""` to omit. |
| `backends.http.lang_param` | — | `""` | Name of the parameter for language filtering. Set to `""` to omit. |
| `backends.http.results_path` | — | `""` | Dot-separated path to the results array in the JSON response. Example: `"data.items"` means the results are at `response.data.items`. Empty = the response itself is the array. |
| `backends.http.title_field` | — | `"title"` | JSON field name for the result title. |
| `backends.http.url_field` | — | `"url"` | JSON field name for the result URL. |
| `backends.http.snippet_field` | — | `"snippet"` | JSON field name for the result snippet/description. |
| `backends.http.headers` | — | `{}` | Static HTTP headers added to every request (e.g. authorization tokens). |
| `backends.http.extra_params` | — | `{}` | Static query parameters added to every request. |

```toml
[backends]
default = "http"

[backends.http]
url           = "https://api.example.com/search"
method        = "GET"
query_param   = "q"
count_param   = "limit"
results_path  = "data.items"
title_field   = "name"
url_field     = "link"
snippet_field = "description"

[backends.http.headers]
"Authorization" = "Bearer my-secret-token"

[backends.http.extra_params]
"format" = "json"
```

---

### LLM settings

These control the optional LLM features: query expansion, summarization,
and LLM-assisted reranking. All LLM features are **disabled by default** —
no data leaves your machine unless you explicitly enable them.

webgate talks to any **OpenAI-compatible API** (OpenAI, Ollama, vLLM,
LM Studio, Together, Groq, etc.). You just need a `/v1/chat/completions`
endpoint.

| TOML key | Env var | CLI arg | Default | What it does |
|----------|---------|---------|---------|--------------|
| `llm.enabled` | `WEBGATE_LLM_ENABLED` | `--llm-enabled` | `false` | Master switch. Set to `true` to activate all enabled LLM features. |
| `llm.base_url` | `WEBGATE_LLM_BASE_URL` | `--llm-base-url` | `"http://localhost:11434/v1"` | Base URL of the OpenAI-compatible API. The default points to a local Ollama instance. |
| `llm.api_key` | `WEBGATE_LLM_API_KEY` | `--llm-api-key` | `""` | API key for authentication. Not needed for local servers like Ollama. |
| `llm.model` | `WEBGATE_LLM_MODEL` | `--llm-model` | `"llama3.2"` | Model name to use (e.g. `"gemma3:27b"`, `"gpt-4o-mini"`, `"llama3.2"`). |
| `llm.timeout` | `WEBGATE_LLM_TIMEOUT` | `--llm-timeout` | `30` | Timeout in seconds for LLM API calls. Increase for slow models or large inputs. |
| `llm.expansion_enabled` | `WEBGATE_LLM_EXPANSION_ENABLED` | `--llm-expansion-enabled` | `true` | **Query expansion.** When you search for one thing, the LLM generates additional related search queries to improve coverage. Example: "rust async" might also search for "tokio tutorial" and "async await patterns". |
| `llm.summarization_enabled` | `WEBGATE_LLM_SUMMARIZATION_ENABLED` | `--llm-summarization-enabled` | `true` | **Summarization.** After fetching and cleaning pages, the LLM writes a Markdown report with `[1]`, `[2]` citations pointing to the source pages. |
| `llm.llm_rerank_enabled` | `WEBGATE_LLM_RERANK_ENABLED` | `--llm-rerank-enabled` | `false` | **LLM reranking.** Uses the LLM to re-sort results by relevance (Tier-2, on top of deterministic BM25). More accurate but costs an extra LLM call. |
| `llm.max_summary_words` | `WEBGATE_LLM_MAX_SUMMARY_WORDS` | `--llm-max-summary-words` | `0` | Maximum words in the summary. `0` = no limit (the LLM decides the length). |
| `llm.input_budget_factor` | `WEBGATE_LLM_INPUT_BUDGET_FACTOR` | `--llm-input-budget-factor` | `3` | Controls how much source text is sent to the LLM. Higher values = more input context but slower and more expensive. |

```toml
[llm]
enabled               = true
base_url              = "http://localhost:11434/v1"
model                 = "gemma3:27b"
expansion_enabled     = true
summarization_enabled = true
llm_rerank_enabled    = false
```

---

## CLI arguments (MCP server only)

These are only available when running `mcp-webgate` (the MCP server binary).
They override both the config file and environment variables.

```
mcp-webgate [OPTIONS]
```

| Argument | What it does |
|----------|--------------|
| `--config <PATH>` | Load config from a specific TOML file instead of auto-discovering `webgate.toml`. |
| `--default-backend <NAME>` | Override the default search backend (e.g. `brave`, `searxng`). |
| `--google-api-key <KEY>` | Google Custom Search API key. |
| `--google-cx <ID>` | Google Custom Search Engine ID. |
| `--bing-api-key <KEY>` | Bing Web Search API key. |
| `--bing-market <CODE>` | Bing market code (e.g. `en-US`). |
| `--debug` | Enable debug logging. |
| `--trace` | Enable trace logging (very verbose). |
| `--log-file <PATH>` | Write logs to a file. |
| `--llm-enabled` | Enable LLM features. |
| `--llm-base-url <URL>` | LLM API base URL. |
| `--llm-api-key <KEY>` | LLM API key. |
| `--llm-model <NAME>` | LLM model name. |
| `--llm-timeout <SECS>` | LLM timeout in seconds. |
| `--llm-expansion-enabled` | Enable query expansion. |
| `--llm-summarization-enabled` | Enable summarization. |
| `--llm-rerank-enabled` | Enable LLM reranking. |
| `--llm-max-summary-words <N>` | Max words in summary. |
| `--llm-input-budget-factor <N>` | Input budget multiplier. |

**Example:**

```bash
mcp-webgate \
  --default-backend brave \
  --llm-enabled \
  --llm-model "gpt-4o-mini" \
  --llm-base-url "https://api.openai.com/v1" \
  --llm-api-key "sk-..."
```

---

## Environment variables quick reference

Every environment variable starts with `WEBGATE_`. Set them in your shell,
in a `.env` file, or in your MCP client configuration.

### Server

| Variable | Type | Default |
|----------|------|---------|
| `WEBGATE_MAX_DOWNLOAD_MB` | number | `1` |
| `WEBGATE_MAX_RESULT_LENGTH` | number | `8000` |
| `WEBGATE_SEARCH_TIMEOUT` | number | `8` |
| `WEBGATE_OVERSAMPLING_FACTOR` | number | `2` |
| `WEBGATE_AUTO_RECOVERY_FETCH` | bool | `false` |
| `WEBGATE_MAX_TOTAL_RESULTS` | number | `20` |
| `WEBGATE_MAX_QUERY_BUDGET` | number | `32000` |
| `WEBGATE_MAX_SEARCH_QUERIES` | number | `5` |
| `WEBGATE_RESULTS_PER_QUERY` | number | `5` |
| `WEBGATE_DEBUG` | bool | `false` |
| `WEBGATE_LOG_FILE` | string | `""` |
| `WEBGATE_TRACE` | bool | `false` |
| `WEBGATE_ADAPTIVE_BUDGET` | `auto`/`on`/`off` | `auto` |
| `WEBGATE_ADAPTIVE_BUDGET_FETCH_FACTOR` | number | `3` |

### Backends

| Variable | Type | Default |
|----------|------|---------|
| `WEBGATE_DEFAULT_BACKEND` | string | `"searxng"` |
| `WEBGATE_SEARXNG_URL` | string | `"http://localhost:8080"` |
| `WEBGATE_BRAVE_API_KEY` | string | `""` |
| `WEBGATE_TAVILY_API_KEY` | string | `""` |
| `WEBGATE_EXA_API_KEY` | string | `""` |
| `WEBGATE_SERPAPI_API_KEY` | string | `""` |
| `WEBGATE_SERPAPI_ENGINE` | string | `"google"` |
| `WEBGATE_SERPAPI_GL` | string | `"us"` |
| `WEBGATE_SERPAPI_HL` | string | `"en"` |
| `WEBGATE_GOOGLE_API_KEY` | string | `""` |
| `WEBGATE_GOOGLE_CX` | string | `""` |
| `WEBGATE_BING_API_KEY` | string | `""` |
| `WEBGATE_BING_MARKET` | string | `"en-US"` |

### LLM

| Variable | Type | Default |
|----------|------|---------|
| `WEBGATE_LLM_ENABLED` | bool | `false` |
| `WEBGATE_LLM_BASE_URL` | string | `"http://localhost:11434/v1"` |
| `WEBGATE_LLM_API_KEY` | string | `""` |
| `WEBGATE_LLM_MODEL` | string | `"llama3.2"` |
| `WEBGATE_LLM_TIMEOUT` | number | `30` |
| `WEBGATE_LLM_EXPANSION_ENABLED` | bool | `true` |
| `WEBGATE_LLM_SUMMARIZATION_ENABLED` | bool | `true` |
| `WEBGATE_LLM_RERANK_ENABLED` | bool | `false` |
| `WEBGATE_LLM_MAX_SUMMARY_WORDS` | number | `0` |
| `WEBGATE_LLM_INPUT_BUDGET_FACTOR` | number | `3` |

Bool values accept: `true`/`1`/`yes` (on) and `false`/`0`/`no` (off).
