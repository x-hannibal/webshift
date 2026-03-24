# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Communication

Chat in Italian, develop and document in English.

## Session Initialization & Tooling

At the start of every session, run Serena's onboarding process to activate
the MCP integration and get access to all LSP-powered tools (code navigation,
symbol search, references, diagnostics, etc.).
Do this before any coding task — it ensures the full toolset is available
and the project context is loaded correctly.

**Once Serena is active, always prefer its tools over CLI alternatives.**

## Project

Native Rust port of [mcp-webgate](../mcp-webgate/) (Python). Denoised web search
library and MCP server. Two crates in a workspace:

- **`webgate`** — library crate (crates.io). Public API: `fetch()`, `query()`, `Config`.
- **`webgate-mcp`** — binary crate (crates.io). MCP server over stdio. Installs as `mcp-webgate`.

## Reference implementation

The Python source lives in `../mcp-webgate/src/mcp_webgate/`. Key files to reference
when porting:

- `scraper/cleaner.py` — lxml XPath cleaning + text sterilization
- `scraper/fetcher.py` — httpx concurrent fetcher, streaming, UA rotation, retry
- `tools/query.py` — full search pipeline
- `tools/fetch.py` — single page fetch
- `config.py` — Pydantic config with env/toml/CLI resolution
- `backends/` — 5 search backend implementations
- `llm/` — OpenAI-compatible client, expander, summarizer
- `utils/reranker.py` — BM25 + LLM reranking
- `utils/url.py` — sanitize, dedup, binary filter

## Stack

- Rust 2024 edition
- `tokio` — async runtime
- `reqwest` — HTTP client (streaming)
- `libxml` — libxml2 bindings for HTML parsing + XPath (same engine as Python lxml)
- `serde` + `toml` — config deserialization
- `regex` — text sterilization
- `clap` — CLI argument parsing
- `rmcp` — MCP server SDK
- `serde_json` — JSON output

## Commands

```bash
cargo build                              # build all
cargo run -p webgate-mcp                 # start MCP server
cargo test                               # run full test suite
cargo test -p webgate                    # test library only
cargo test -p webgate -- test_name       # run a single test
```

## Build dependencies

libxml2 must be installed on the system:

```bash
# Linux
sudo apt install libxml2-dev pkg-config

# macOS
brew install libxml2

# Windows
vcpkg install libxml2:x64-windows
# set VCPKG_ROOT=C:\vcpkg
```

## Architecture

```
crates/
  webgate/                   # library crate
    src/
      lib.rs                 # public API: fetch(), query(), Config
      config.rs              # serde config (toml + env + CLI)
      scraper/
        fetcher.rs           # reqwest concurrent fetcher, streaming cap, UA rotation
        cleaner.rs           # libxml2 HTML cleaning + regex text sterilization
      backends/
        mod.rs               # SearchBackend trait + SearchResult
        searxng.rs, brave.rs, tavily.rs, exa.rs, serpapi.rs
      llm/
        client.rs            # OpenAI-compatible async client
        expander.rs          # query expansion via LLM
        summarizer.rs        # Markdown report with citations
      utils/
        url.rs               # sanitize, dedup, binary filter, domain filter
        reranker.rs          # BM25 deterministic + LLM reranking

  webgate-mcp/               # binary crate → installs as `mcp-webgate`
    src/
      main.rs                # MCP tool registration, stdio transport
```

## Anti-flooding protections (DO NOT remove)

These are the core value proposition — must exist in every code path:

- `max_download_mb`: streaming cap on per-page download (never buffer full response)
- `max_result_length`: hard cap on chars per cleaned page
- `max_query_budget`: total char budget across all sources
- `max_total_results`: hard cap on results per call
- Binary extension filter runs BEFORE any network request
- Streaming in fetcher — **DO NOT use `response.text()`**, use `bytes_stream()` with size check
- Unicode/BiDi sterilization in cleaner

## Configuration

Same resolution order as Python: **CLI args > env vars (`WEBGATE_*`) > `webgate.toml` > defaults**.
Same env var names, same TOML structure — drop-in compatible config files.

## Commit convention

Do NOT add Co-Authored-By tags to commit messages.

Format: `type(scope): description`

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`.

Examples:
```
feat(cleaner): port lxml XPath cleaning pipeline
fix(fetcher): respect Retry-After header on 429
chore(deps): add reqwest with streaming feature
```
