# Under the Hood

How webshift turns a raw search query into clean, budget-capped, LLM-ready text.

---

## Pipeline overview

```
query("model context protocol overview")
        │
        ▼
┌─────────────────────┐
│  0. URL pre-filter  │  binary ext / domain allow-list / block-list
└──────────┬──────────┘
           │
           ▼
┌──────────────────────┐
│  1. Query expansion  │  LLM → 1 query → N complementary queries   [opt-in]
└──────────┬───────────┘
           │
           ▼
┌────────────────────────────────┐
│  2. Backend search (parallel)  │  oversample Nx per query → round-robin flatten
└──────────────┬─────────────────┘
               │
               ▼
┌──────────────────────────────────────────┐
│  3. Fetch (concurrent, streaming cap)    │  Round 1: candidates
│                                          │  Round 2: gap-fill from reserve pool
└───────────────┬──────────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│  4. HTML denoising (two-stage)        │  Stage A: tag removal
│                                      │  Stage B: text sterilization
└───────────────┬──────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  5. Tier-1 rerank: deterministic BM25      │  always active, zero cost
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  6. Adaptive budget allocation             │  proportional to BM25 scores  [opt-in]
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  7. Tier-2 rerank: LLM-assisted            │  lightweight title+snippet prompt  [opt-in]
└───────────────┬────────────────────────────┘
                │
                ▼
┌────────────────────────────────────────────┐
│  8. LLM summarization                      │  Markdown report with citations   [opt-in]
└────────────────────────────────────────────┘
```

---

## Stage 0 — URL pre-filter

Before any network request:

- **Binary extension filter** — rejects URLs ending in `.pdf`, `.zip`, `.exe`, `.mp4`, etc.
  Nothing is downloaded; the URL is simply dropped from the candidate list.
- **Domain allow/block list** — respects `allowed_domains` / `blocked_domains` from config.

This is the cheapest possible filter and runs unconditionally.

---

## Stage 1 — Query expansion (opt-in, requires `llm` feature)

When `llm.expansion_enabled = true` and exactly one query is passed:

```
"model context protocol overview"
        │  LLM prompt: "generate N complementary queries for …"
        ▼
[
  "model context protocol overview",
  "large language model context window size",
  "retrieval augmented generation protocol",
  "model context management best practices",
  "vector database context retrieval"
]
```

The original query is always kept as the first element.
Multiple queries increase recall by covering adjacent vocabulary
(the BM25 rerank at Stage 5 then pulls the most relevant results to the top).

Config knobs: `llm.expansion_enabled`, `server.max_search_queries` (caps the number of expanded queries).

---

## Stage 2 — Backend search + oversampling

Each expanded query is issued **in parallel** to the configured backend (SearXNG, Brave, etc.).

**Oversampling** — each query requests `N × oversampling_factor` results from the backend
(default factor = 2).  With 5 queries × 5 results × 2x oversampling = up to 50 candidates are
gathered before dedup.  The extra results are kept in a *reserve pool* (snippet-only) for:

- Gap-filling (Stage 3)
- The `snippet_pool` field in `QueryResult` — available to callers for additional context

After all backends respond, results are merged in **round-robin order**
(Q1[0], Q2[0], Q3[0], …, Q1[1], Q2[1], …) so no single query dominates the candidate list.
URL deduplication and the binary/domain filter run at this point.

---

## Stage 3 — Parallel fetch with anti-flooding protections

**Round 1** — the top `max_total_results` candidates are fetched concurrently.

Each fetch:
- Streams the response via `bytes_stream()` — **never buffers the full response**
- Hard-stops at `max_download_mb` (default 1 MB) per page regardless of `Content-Length`
- Respects `search_timeout` (default 8 s)

**Round 2 — gap fill** — if some Round 1 fetches fail (timeout, 404, bot-block),
they are replaced with backup URLs from the reserve pool, fetched immediately.
Truly failed sources fall back to their search snippet.

The timing map records `(elapsed_ms, raw_bytes)` per URL — used for the
`Stats.raw_bytes` compression metric.

---

## Stage 4 — HTML denoising

Two distinct passes run sequentially on every fetched page.

### Stage 4A — Tag-level noise removal (structural)

Uses `scraper` / `html5ever` (pure Rust) to parse the DOM.
Text nodes are extracted **only if no ancestor is a noise element**:

```
script  style  nav  footer  header  aside  form
iframe  noscript  svg  button  input  select  textarea
```

This removes JS, CSS, navigation bars, footers, cookie banners, forms, and SVG content
in one DOM traversal — no regex, no heuristic line counting.

### Stage 4B — Text sterilization (lexical)

The raw text string produced by Stage 4A goes through a regex/char-map pipeline:

| Step | What it removes / replaces |
|------|---------------------------|
| Unicode junk | C0/C1 controls, zero-width chars, BiDi override chars (`\u202a`–`\u202e`, `\u2066`–`\u2069`) |
| Typography normalization | Smart quotes → `'`/`"`, em-dash → ` - `, soft hyphen → dropped, ellipsis → `...`, ligatures (fi, fl, …) → ASCII |
| Whitespace collapse | tabs, non-breaking spaces → single space |
| Noise line filter | Exact-match regex against known boilerplate words: `menu`, `sign in`, `subscribe`, `cookie`, `advertisement`, `follow us`, `read more`, `loading`, … |
| Duplicate line removal | Consecutive identical lines collapsed to one |
| Date-only lines | Bare date strings (`01/12/2024`, `Jan 1, 2024`) stripped |
| Multi-newline collapse | 3+ consecutive newlines → double newline |

**Snippet fallback** — if the cleaned page is empty (aggressive cookie wall, JS-only SPA,
paywalled content), the source falls back to the search engine's own snippet.

---

## Stage 5 — Tier-1 rerank: deterministic BM25

All cleaned sources are scored against the full query set using **BM25 (k1=1.5, b=0.75)**.

### BM25 formula

For each document *d* and query term *t*:

```
IDF(t)  = ln( (N - df(t) + 0.5) / (df(t) + 0.5) + 1 )
TF_norm = tf(t,d) × (k1 + 1) / ( tf(t,d) + k1 × (1 - b + b × |d| / avgdl) )
score(d) = Σ_t  IDF(t) × TF_norm(t, d)
```

Where:
- *N* = number of documents in this result set
- *df(t)* = how many documents contain term *t*
- *tf(t,d)* = raw term count in document *d*
- *|d|* = token count of document *d*
- *avgdl* = average token count across all documents

The BM25 document string for each source is: `title + snippet + first 3000 chars of content`
(multibyte-safe; uses `.chars().take(3000)` — not a byte slice).

Sources are sorted descending by score. **This is fully deterministic** — same input always
produces the same ranking.

### Real example (from harness run)

Query: `"model context protocol overview"` (expanded to 5 queries).

```
 id  score   bm25           chars  budget  trunc
──────────────────────────────────────────────
[1]  9.7129  ████████████    3200    3200    yes   ← Cloudflare Vectorize docs
[2]  7.3289  █████████░░░    3200    3200    yes   ← Wikipedia RAG
[3]  5.2898  ███████░░░░░     335    3200     no   ← DataCamp context window
[4]  3.1246  ████░░░░░░░░    2640    3200     no   ← MCP official docs
[5]  0.7545  █░░░░░░░░░░░     492    3200     no   ← Anthropic context engineering
```

Source [1] (vector databases) scored highest because its content densely matched
multiple expanded query terms (`context`, `retrieval`, `vector`).
Source [5] ranked last despite being arguably the most directly relevant page —
its short preview had low term overlap with the expanded vocabulary.
This is the classic BM25 vocabulary mismatch problem, which Tier-2 LLM rerank addresses.

---

## Stage 6 — Adaptive budget allocation

The total character budget (`max_query_budget`) must be divided among all sources.
There are two strategies: **flat** (equal shares) and **adaptive** (proportional
to BM25 scores). The `adaptive_budget` setting controls which one is used.

### `adaptive_budget = "auto"` (default)

webshift decides automatically by computing the **dominance ratio**:

```
dominance_ratio = (max_score / total_score) × number_of_sources
```

- If dominance > 1.5 → the top source is significantly more relevant → **adaptive ON**
- If dominance <= 1.5 → scores are fairly uniform → **flat allocation** (same result anyway)

In the harness, the resolved decision is shown as:

```
adaptive budget:  auto (→ on,  dominance 1.85)
```

### Flat allocation (`adaptive_budget = "off"`)

```
per_page_limit = min(max_result_length, max_query_budget / num_sources)
              = min(8000, 16000 / 5) = 3200 chars per source
```

Every source gets the same cap regardless of relevance score.

### Adaptive allocation (`adaptive_budget = "on"`)

The fetcher uses a larger cap during Stage 3 (`max_result_length × adaptive_budget_fetch_factor`,
default 3x) to download more text upfront.  After BM25 scoring:

```
alloc(source_i) = (score_i / total_score) × total_budget
```

With the example scores above:
```
total_score = 9.71 + 7.33 + 5.29 + 3.12 + 0.75 = 26.20

[1]: 9.71 / 26.20 × 16000 ≈ 5933 chars  (flat would give 3200)
[2]: 7.33 / 26.20 × 16000 ≈ 4476 chars
[3]: 5.29 / 26.20 × 16000 ≈ 3230 chars
[4]: 3.12 / 26.20 × 16000 ≈ 1905 chars
[5]: 0.75 / 26.20 × 16000 ≈  458 chars  (flat would give 3200)
```

The top source gets almost twice the budget it would under flat allocation.
The bottom source gets much less — but it only had 492 characters anyway,
so nothing is lost.

A **surplus redistribution** pass (up to 5 iterations) then reclaims unspent budget
from short sources and gives it proportionally to sources that were truncated.

### When does auto mode help?

- **Heterogeneous results** (one great source + several mediocre ones): auto detects
  the spread and gives the great source more room. The `dominance_ratio` in the
  example (1.85) correctly triggers adaptive mode.
- **Uniform results** (all equally relevant): auto stays flat — redistributing
  would just shuffle a few characters around with no real benefit.

---

## Stage 7 — Tier-2 rerank: LLM-assisted (opt-in)

When `llm.llm_rerank_enabled = true`, each source is represented as a
lightweight line (`[id] title — first 200 chars`) and sent to the LLM
with a single prompt:

```
Rank the following search results by relevance to the query: "…"
Output only a JSON array of IDs in order from most to least relevant.

[1] Cloudflare Vectorize docs — Vector databases are a key part…
[2] Wikipedia RAG — Retrieval-augmented generation is a technique…
…
```

The LLM returns `[4, 5, 2, 1, 3]` and the sources are reordered accordingly.
Any source the LLM omits is appended at the end (safe fallback).
On any error (timeout, bad JSON, HTTP 500) the Tier-1 order is preserved.

This corrects the BM25 vocabulary mismatch problem illustrated in the Stage 5 example:
the MCP official docs (source [4]) and the Anthropic context engineering post (source [5])
would likely move to positions [1] and [2] after LLM rerank.

---

## Stage 8 — LLM summarization (opt-in)

When `llm.summarization_enabled = true`, all `sources[].content` strings are
concatenated with citation markers and sent to the LLM:

```
[1] https://cloudflare.com/…
<content>

[2] https://en.wikipedia.org/…
<content>
…
```

The LLM produces a structured Markdown report with inline `[N]` citations.
The summary is returned in `QueryResult.summary`.

**Cross-language normalization (bonus):** if BM25 reranking surfaces pages
in foreign languages (Chinese, Japanese, Arabic, …), the LLM summarizer still
produces the final report in the prompt language — the model translates implicitly.

### Compression chain (real example)

```
raw download:     574.0 KB   ← streaming HTTP bytes (5 pages × ~115 KB avg)
clean text:        9.6 KB   (98.3% reduction)  ← after denoising + per-page cap
llm summary:       5.2 KB   (99.1% reduction)  ← final Markdown report
```

574 KB of HTML → 5.2 KB of structured, cited, on-topic text delivered to the LLM context.

---

## Configuration reference

| Key | Default | Effect |
|-----|---------|--------|
| `server.max_total_results` | `20` | Hard cap on sources returned |
| `server.max_query_budget` | `32000` | Total char budget across all sources |
| `server.max_result_length` | `8000` | Hard per-page char cap before budget math |
| `server.max_download_mb` | `1` | Streaming cap per HTTP response |
| `server.search_timeout` | `8` | Seconds per fetch |
| `server.language` | `"en"` | BCP-47 hint passed to all backends (empty = let backend decide) |
| `server.oversampling_factor` | `2` | Oversampling multiplier for reserve pool |
| `server.adaptive_budget` | `"auto"` | Budget allocation mode: `"auto"` / `"on"` / `"off"` (see Stage 6) |
| `server.adaptive_budget_fetch_factor` | `3` | Fetch cap multiplier when adaptive is on |
| `server.auto_recovery_fetch` | `false` | Gap-fill failed fetches from reserve pool |
| `llm.expansion_enabled` | `true` | Multi-query expansion via LLM |
| `llm.summarization_enabled` | `true` | Markdown report with citations |
| `llm.llm_rerank_enabled` | `false` | LLM-assisted Tier-2 rerank |
| `llm.max_summary_words` | `0` | Word cap in summarization prompt (0 = unlimited) |
| `llm.input_budget_factor` | `3` | Max input tokens = output budget × this factor |
