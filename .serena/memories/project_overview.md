# rs-webgate Project Overview

Native Rust port of mcp-webgate (Python). Denoised web search library and MCP server.

## Workspace Crates
- **webgate** (library, crates.io) — public API: `clean()`, `fetch()`, `query()`, `Config`
- **webgate-mcp** (binary, crates.io) — MCP server over stdio, installs as `mcp-webgate`
- **robot** (internal dev tool) — bump, test, promote, unpromote, publish

## Tech Stack
- Rust 2024 edition, tokio async, reqwest HTTP, scraper (html5ever), serde+toml config
- rmcp (Anthropic MCP SDK 1.x), clap CLI, regex text sterilization
- Pure Rust — no C system dependencies

## Feature Flags (webgate crate)
- `backends` (default on) — 5 search backends + query pipeline
- `llm` (default off) — LLM client, expander, summarizer, LLM reranking

## Key Design Principles
- Anti-flooding: streaming download caps, result length caps, budget caps, binary filter before requests
- Config resolution: CLI > env (WEBGATE_*) > webgate.toml > defaults
- Drop-in compatible with Python version's config files
