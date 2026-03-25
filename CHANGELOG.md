# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.1.8] - 2026-03-25

### Added

- `backends/google.rs` — Google Custom Search API backend (`api_key` + `cx`; free tier: 100 req/day)
- `backends/bing.rs` — Bing Web Search API backend (`api_key` + `market`; free tier: 1,000 req/month)
- `backends/http.rs` — generic configurable HTTP backend: point at any REST search API via TOML alone (no Rust code required)
- `GoogleConfig`, `BingConfig`, `HttpBackendConfig` in `config.rs`
- CLI args: `--google-api-key`, `--google-cx`, `--bing-api-key`, `--bing-market`
- Factory tests for new backends (3 tests: Google/Bing need API key, HTTP needs URL)
- `docs/integrations/IDE.md` — client-specific setup for Claude Desktop, Claude Code, Zed, Cursor, Windsurf, VS Code
- `docs/integrations/AGENT.md` — CLI agent setup for Gemini CLI, Claude CLI, custom agents

### Changed

- `README.md` restructured — slim overview with links to `docs/integrations/`
- `CHANGELOG.md` rewritten in [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format
- Backend list in onboarding guide updated: `searxng | brave | tavily | exa | serpapi | google | bing | http`

### Fixed

- `crates/robot/Cargo.toml` corruption (`58890-` stray line removed, missing deps restored)

---

## [0.1.7] - 2026-03-25

### Added

- Integration test infrastructure: `test.toml` config with per-backend and LLM `enabled` flags for live tests
- `TestConfig` struct with `to_webgate_config()` conversion (shared by tests and harness)
- Live integration tests: 5 backend tests, 3 LLM pipeline tests, 1 fetch test (all `#[ignore]`, require `test.toml`)
- `robot harness` subcommand — full pipeline runner with BM25 scores, budget stats, timing, consolidated report
- Example configs: `webgate.toml`, `webgate-ollama.toml`, `webgate-minimal.toml`, `webgate-brave.toml`
- Example MCP client configs: `claude-desktop.json`, `claude-desktop-ollama.json`
- `test.toml.example` template for contributor integration testing

### Changed

- `README.md` updated with comprehensive installation, configuration, backends, and LLM documentation

---

## [0.1.6] - 2026-03-25

### Added

- `LlmClient` — async OpenAI-compatible chat completions client (reqwest, no SDK dependency)
- `expand_queries()` — single query → N complementary queries via LLM, with JSON fence stripping and fallback
- `summarize_results()` — Markdown report with inline citations `[1]`, `[2]`, etc.
- `rerank_llm()` — LLM-assisted tier-2 reranking (behind `llm` feature flag), falls back to input order on error
- LLM expansion integrated in query pipeline (single query input, `llm.expansion_enabled`)
- LLM reranking integrated after BM25 (`llm.llm_rerank_enabled`)
- LLM summarization integrated after reranking (`llm.summarization_enabled`); errors captured in `llm_summary_error` field
- `webgate-mcp` now builds with `llm` feature enabled
- CLI args for all LLM settings: `--llm-enabled`, `--llm-model`, `--llm-base-url`, `--llm-api-key`, `--llm-timeout`, `--llm-expansion-enabled`, `--llm-summarization-enabled`, `--llm-rerank-enabled`, `--llm-max-summary-words`, `--llm-input-budget-factor`
- Tests: `LlmClient` (4), expander (4), summarizer (2), `rerank_llm` (2), LLM pipeline (3), MCP LLM CLI args (1)

---

## [0.1.5] - 2026-03-25

### Added

- `SearchBackend` trait + `create_backend` factory with 5 implementations: SearXNG, Brave, Tavily, Exa, SerpAPI
- BM25 deterministic reranking + adaptive budget redistribution
- Full query pipeline: search → dedup → fetch → clean → rerank → assemble with oversampling and gap filler
- `webgate::query()` and `webgate::query_with_options()` public library API
- `webgate_query` MCP tool with `StringOrList` queries param, backend override, and lang support
- Tests: backend factory (4), SearXNG wiremock (4), BM25 reranker (6), pipeline integration (8), MCP query params (3)

---

## [0.1.4] - 2026-03-25

### Added

- MCP server tests: construction, onboarding JSON, CLI parsing, param deserialization (10 tests)

### Changed

- `robot bump` now auto-commits all tracked changes, not just `Cargo.toml` and `CHANGELOG.md`

---

## [0.1.3] - 2026-03-25

### Added

- `webgate_fetch` MCP tool via `rmcp` 1.x with stdio transport
- `webgate_onboarding` MCP tool — operational guide JSON (matches Python implementation)
- CLI argument parsing with clap: `--config`, `--default-backend`, `--debug`, `--trace`, `--log-file`
- MCP server instructions for AI agent guidance
- `tracing-subscriber` logging to stderr or file

---

## [0.1.2] - 2026-03-25

### Added

- TOML config loading + `WEBGATE_*` env var overrides
- Cleaner test suite: port of full Python test cases (12 tests)
- Fetcher tests: wiremock retry — 429, 503, exhausted retries, 404 no-retry (6 tests)
- Config tests: TOML parsing, env override, CLI override

---

## [0.1.1] - 2026-03-25

### Added

- Workspace scaffold: `webgate` (lib), `webgate-mcp` (bin), `robot` (dev tool)
- `robot` commands: `bump`, `test`, `promote`, `unpromote`, `publish`
- HTML cleaner with `scraper`/html5ever + text sterilization pipeline
- `reqwest` concurrent fetcher with streaming cap, UA rotation, and retry
- URL utils: sanitize, dedup, binary extension filter
- `webgate::clean()` and `webgate::fetch()` public API
- Release profile with LTO, strip, and size optimization
- `CLAUDE.md`, `CONTRIBUTING.md`, `PLAN.md`

[Unreleased]: https://github.com/annibale-x/webgate/compare/v0.1.8...HEAD
[0.1.8]: https://github.com/annibale-x/webgate/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/annibale-x/webgate/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/annibale-x/webgate/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/annibale-x/webgate/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/annibale-x/webgate/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/annibale-x/webgate/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/annibale-x/webgate/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/annibale-x/webgate/releases/tag/v0.1.1
