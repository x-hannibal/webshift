# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Communication

Chat in Italian, develop and document in English.

## MANDATORY Session Initialization & Tooling

At the start of every session, run Serena's onboarding process to activate
the MCP integration and get access to all LSP-powered tools (code navigation,
symbol search, references, diagnostics, etc.).
Do this before any coding task — it ensures the full toolset is available
and the project context is loaded correctly.

**Once Serena is active, always prefer its tools over CLI alternatives.**

## Project

Native Rust port of [mcp-webshift](../mcp-webshift/) (Python). Denoised web search
library and MCP server. Two crates in a workspace:

- **`webshift`** — library crate (crates.io). Public API: `clean()`, `fetch()`, `query()`, `Config`.
- **`webshift-mcp`** — binary crate (crates.io). MCP server over stdio. Installs as `mcp-webshift`.
- **`robot`** — internal dev tool (not published). `bump`, `test`, `promote`, `unpromote`, `publish`.

## Reference implementation

The Python source lives in `../mcp-webshift/src/mcp_webshift/`. Key files to reference
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
- `scraper` (html5ever) — pure-Rust HTML parsing + CSS selector noise removal
- `serde` + `toml` — config deserialization
- `regex` — text sterilization
- `clap` — CLI argument parsing
- `rmcp` — MCP server SDK (official Anthropic Rust SDK, 1.x stable)
- `serde_json` — JSON output

**No C system dependencies.** Pure Rust throughout — enables static binaries on all targets without system package requirements.

## Commands

```bash
cargo build                              # build all
cargo run -p webshift-mcp                 # start MCP server
cargo test                               # run full test suite
cargo test -p webshift                    # test library only
cargo test -p webshift -- test_name       # run a single test
```

## Architecture

```
crates/
  webshift/                   # library crate (published to crates.io)
    src/
      lib.rs                 # public API: clean(), fetch(), query(), Config
      config.rs              # serde config (toml + env + CLI)
      scraper/
        fetcher.rs           # reqwest concurrent fetcher, streaming cap, UA rotation
        cleaner.rs           # scraper/html5ever HTML cleaning + regex text sterilization
      backends/              # feature: "backends" (default on)
        mod.rs               # SearchBackend trait + SearchResult
        searxng.rs, brave.rs, tavily.rs, exa.rs, serpapi.rs, google.rs, bing.rs, http.rs
      llm/                   # feature: "llm" (default off)
        client.rs            # OpenAI-compatible async client
        expander.rs          # query expansion via LLM
        summarizer.rs        # Markdown report with citations
      utils/
        url.rs               # sanitize, dedup, binary filter, domain filter
        reranker.rs          # BM25 deterministic (+ LLM reranking with "llm" feature)

  webshift-mcp/               # binary crate (published to crates.io) → installs as `mcp-webshift`
    src/
      main.rs                # MCP tool registration, stdio transport

  robot/                     # internal dev tool (publish = false)
    src/
      main.rs                # bump, test, promote, unpromote, publish
```

### Feature flags (`webshift` crate)

| Feature | Default | Enables |
|---------|---------|---------|
| `backends` | on | All 8 search backends + query pipeline |
| `llm` | off | LLM client, query expander, summarizer, LLM reranking |

Minimal dependency (cleaner + fetcher only):
```toml
webshift = { version = "x.y.z", default-features = false }
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

Same resolution order as Python: **CLI args > env vars (`WEBSHIFT_*`) > `webshift.toml` > defaults**.
Same env var names, same TOML structure — drop-in compatible config files.

## Versioning & Release

All crates share a single version via `[workspace.package] version` in the root `Cargo.toml`.

**When asked to bump the version:**
1. Update `CHANGELOG.md` with all changes since the last release (Added / Changed / Fixed / Removed sections).
2. Run `cargo run -p robot -- bump [X.Y.Z]` (omit version to auto-increment Z).

The `bump` command updates `Cargo.toml` workspace version and commits with `chore(release): bump to X.Y.Z`.

**Release flow:**
```bash
cargo run -p robot -- bump          # update CHANGELOG + bump version
cargo run -p robot -- test          # cargo test all crates
cargo run -p robot -- promote       # build + test + merge dev→main + tag + push + checkout dev
cargo run -p robot -- publish       # (M5+) cargo publish both crates to crates.io
cargo run -p robot -- unpromote     # undo last promote if needed
```

## Progress tracking

When completing tasks from PLAN.md milestones, check them off (`- [x]`) immediately.

## Local services

- **SearXNG** — default port is `http://localhost:8080`. Use it for integration tests
  in M3 (query pipeline development) and as the default backend during development.

## Commit convention

Do NOT add Co-Authored-By tags to commit messages.

Git commit messages use `type(scope): description` format.

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`.

Examples:
```
feat(query): add oversampling gap filler
fix(cleaner): resolve BiDi regex false positive
chore(deps): bump mcp to 1.3.0
```

## Changelog convention

When performing "BUMP vX.Y.Z", update both `CHANGELOG.md` and `README.md` following the **Keep a Changelog** standard.
  - **Header**: Use the format `## [X.Y.Z] - YYYY-MM-DD`.
  - **Structure**: Categorize all changes under the following semantic subsections: `### Added`, `### Changed`, `### Deprecated`, `### Fixed`, `### Removed`, or `### Security`.
  - **Style**: Use plain indented bullet points for each change. Do not use bold text within the descriptions. 
  - **Sync**: Ensure the version number and the latest "Added/Fixed" highlights are synchronized in the `README.md`.
