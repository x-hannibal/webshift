# Use Cases and Examples

Real-world configurations for common scenarios. Each example is a complete
`webshift.toml` you can copy and adjust.

---

## 1. Local-first with SearXNG (no API keys, no LLM)

**Who is this for?** You want web search for your agent but don't want to pay
for API keys or send data to external services. Everything runs on your machine.

**What you need:** Docker (for SearXNG).

```bash
# Start SearXNG
docker run -d -p 8080:8080 searxng/searxng
```

```toml
# webshift.toml
[backends]
default = "searxng"

[backends.searxng]
url = "http://localhost:8080"
```

**What you get:** raw search results with cleaned text. No summaries,
no query expansion. Fast and private.

---

## 2. Local SearXNG + local LLM (fully offline)

**Who is this for?** You want the full pipeline (search + summaries) but
everything must stay on your machine. No internet API calls except the
search itself.

**What you need:** Docker (for SearXNG) + [Ollama](https://ollama.ai/) with
a model pulled.

```bash
docker run -d -p 8080:8080 searxng/searxng
ollama pull gemma3:27b
```

```toml
# webshift.toml
[server]
max_total_results = 5
max_query_budget  = 16000
language          = "en"

[backends]
default = "searxng"

[backends.searxng]
url = "http://localhost:8080"

[llm]
enabled               = true
base_url              = "http://localhost:11434/v1"
model                 = "gemma3:27b"
expansion_enabled     = true     # one question → multiple search queries
summarization_enabled = true     # get a Markdown report with citations
```

**What you get:** the agent asks one question, webshift expands it into
5 related queries, searches all of them, fetches the top pages, cleans
them, ranks them by relevance, and produces a cited Markdown summary.
All locally.

---

## 3. Cloud backend (Brave) + cloud LLM (OpenAI)

**Who is this for?** You don't want to self-host anything. You have API keys
and want the best quality.

```toml
# webshift.toml
[backends]
default = "brave"

[backends.brave]
api_key = "BSA-xxxxxxxxxxxx"

[llm]
enabled               = true
base_url              = "https://api.openai.com/v1"
api_key               = "sk-xxxxxxxxxxxx"
model                 = "gpt-4o-mini"
timeout               = 60
expansion_enabled     = true
summarization_enabled = true
llm_rerank_enabled    = true    # use LLM to improve ranking accuracy
```

**What you get:** maximum quality. LLM expansion + BM25 reranking +
LLM reranking + LLM summary. More API calls, but most accurate results.

---

## 4. Multiple backends available (switch per query)

**Who is this for?** You have several API keys and want the flexibility
to choose the best backend for each query.

```toml
# webshift.toml
[backends]
default = "searxng"       # used when no backend is specified

[backends.searxng]
url = "http://localhost:8080"

[backends.brave]
api_key = "BSA-xxxxxxxxxxxx"

[backends.google]
api_key = "AIza..."
cx      = "a1b2c3..."

[backends.bing]
api_key = "xxxxxxxxxxxx"
market  = "en-US"
```

The agent can override the backend per query:

```json
{ "queries": "latest rust features", "backend": "brave" }
{ "queries": "site:docs.rs tokio runtime", "backend": "google" }
```

---

## 5. Tight budget (minimal context usage)

**Who is this for?** Your LLM has a small context window (4K-8K tokens)
or you want to minimize token usage to save costs.

```toml
# webshift.toml
[server]
max_total_results = 3         # only 3 pages
max_query_budget  = 4000      # tiny budget: ~1000 tokens
max_result_length = 2000      # each page capped at 2000 chars
max_search_queries = 2        # limit query expansion

[backends]
default = "searxng"

[backends.searxng]
url = "http://localhost:8080"
```

**What you get:** very compact results. 3 pages, 4000 characters total.
Fits easily in a small context window.

---

## 6. Large budget (research mode)

**Who is this for?** You have a 128K+ context window and want comprehensive
results for deep research.

```toml
# webshift.toml
[server]
max_total_results   = 20
max_query_budget    = 64000     # 64K chars ≈ 16K tokens
max_result_length   = 8000
max_search_queries  = 5
results_per_query   = 10
oversampling_factor = 3         # large reserve pool
auto_recovery_fetch = true      # replace failed fetches automatically
adaptive_budget     = "on"      # give more budget to better sources

[backends]
default = "brave"

[backends.brave]
api_key = "BSA-xxxxxxxxxxxx"

[llm]
enabled               = true
base_url              = "http://localhost:11434/v1"
model                 = "gemma3:27b"
expansion_enabled     = true
summarization_enabled = true
llm_rerank_enabled    = true
```

**What you get:** 20 pages, 64K chars of context, LLM reranking,
adaptive budget allocation, gap-fill recovery. Maximum depth.

---

## 7. Environment variables only (no config file)

**Who is this for?** You're running webshift in a container or CI/CD pipeline
where files are inconvenient.

```bash
export WEBSHIFT_DEFAULT_BACKEND=brave
export WEBSHIFT_BRAVE_API_KEY=BSA-xxxxxxxxxxxx
export WEBSHIFT_MAX_TOTAL_RESULTS=5
export WEBSHIFT_MAX_QUERY_BUDGET=16000
export WEBSHIFT_LLM_ENABLED=true
export WEBSHIFT_LLM_BASE_URL=https://api.openai.com/v1
export WEBSHIFT_LLM_API_KEY=sk-xxxxxxxxxxxx
export WEBSHIFT_LLM_MODEL=gpt-4o-mini
```

No `webshift.toml` needed. Everything is configured from the environment.

---

## 8. Custom HTTP backend (your own search API)

**Who is this for?** You have an internal search API or a third-party
service not natively supported by webshift.

Suppose your API works like this:

```
GET https://search.internal.corp/api/find?query=rust+async&limit=10
→ { "results": [ { "name": "...", "link": "...", "desc": "..." } ] }
```

```toml
# webshift.toml
[backends]
default = "http"

[backends.http]
url           = "https://search.internal.corp/api/find"
method        = "GET"
query_param   = "query"
count_param   = "limit"
results_path  = "results"
title_field   = "name"
url_field     = "link"
snippet_field = "desc"

[backends.http.headers]
"X-API-Key" = "internal-secret-123"
```

**What you get:** webshift talks to your custom API using the field
mappings you defined. No code changes needed.

---

## 9. MCP client configuration (Claude Code)

**Who is this for?** You're using Claude Code (or any MCP-compatible client)
and want to configure webshift via the MCP server settings.

In your `claude_desktop_config.json` or equivalent:

```json
{
  "mcpServers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": [
        "--default-backend", "searxng",
        "--llm-enabled", "true",
        "--llm-model", "gemma3:27b"
      ],
      "env": {
        "WEBSHIFT_SEARXNG_URL": "http://localhost:8080"
      }
    }
  }
}
```

CLI args and env vars can be mixed. CLI args take priority.

---

## 10. Domain filtering

**Who is this for?** You want to exclude low-quality domains from results,
or restrict results to trusted sources only.

### Block specific domains

```toml
[server]
blocked_domains = [
  "pinterest.com",
  "quora.com",
  "reddit.com"
]
```

### Allow only specific domains

```toml
[server]
allowed_domains = [
  "docs.rs",
  "doc.rust-lang.org",
  "github.com",
  "stackoverflow.com"
]
```

When `allowed_domains` is set, **only** URLs from those domains are fetched.
Everything else is silently dropped.
