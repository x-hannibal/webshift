# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.2.2] - 2026-03-26

### Fixed

- All relative links in README.md converted to absolute GitHub URLs for crates.io compatibility
- Library version in README installation instructions updated from 0.1 to 0.2

---

## [0.2.1] - 2026-03-25

### Added

- Comprehensive rustdoc documentation for all public types, fields, and functions in the `webshift` library crate
- Crate-level doc with feature flags table, three usage examples (clean, fetch, query), and anti-flooding overview
- Field-level doc comments on `CleanResult`, `FetchResult`, `Source`, `SnippetEntry`, `Stats`, `QueryResult`, `WebshiftError`, `Config`, `ServerConfig`, `AdaptiveBudget`, `BackendsConfig`, `LlmConfig`
- `readme`, `keywords`, and `categories` fields in both crate Cargo.toml files for crates.io display
- 3 new doctests from inline code examples

### Changed

- `backends/mod.rs` module doc updated to list all 8 backends (was 5)

---

## [0.2.0] - 2026-03-25

### Changed

- Project renamed from `webgate` to `webshift` across the entire codebase
- Crate names: `webgate` → `webshift`, `webgate-mcp` → `webshift-mcp`
- Binary name: `mcp-webgate` → `mcp-webshift`
- Config file: `webgate.toml` → `webshift.toml`
- Environment variable prefix: `WEBGATE_*` → `WEBSHIFT_*`
- MCP tool names: `webshift_onboarding`, `webshift_fetch`, `webshift_query`
- Error type: `WebgateError` → `WebshiftError`
- Repository moved to `github.com/annibale-x/webshift`

### Added

- GitHub Actions CI workflow (`ci.yml`): test on ubuntu/windows/macos with clippy
- GitHub Actions release workflow (`release.yml`): cross-compile 5 targets, GitHub Release with prebuilt binaries
- docs.rs annotations: `#[doc(cfg(feature = "..."))]` on feature-gated modules (`backends`, `llm`)
- `[package.metadata.docs.rs]` configured with `all-features = true`
- All clippy warnings resolved (`derivable_impls`, `collapsible_if`, `manual_flatten`, `map_or`)
- First publish to crates.io: `webshift` (library) + `webshift-mcp` (MCP server binary)

---

## [0.1.12] - 2026-03-25

### Added

- 95 new tests across all crates, bringing total from 104 to ~200
- Wiremock-based test suites for all 7 previously untested backends: Brave (8), Tavily (6), Exa (7), SerpAPI (5), Google (7), Bing (7), HTTP (13)
- Public API tests for `clean()`, `fetch()`, and `query()` in lib.rs (9 tests covering fields, truncation, noise removal, binary/domain filters)
- Config tests for AdaptiveBudget deserialization (bool/string variants), env_bool helper, language default (10 tests)
- Edge case tests for reranker (empty inputs, no surplus), LLM client (empty choices, invalid JSON), expander (non-array fallback, n cap), summarizer (empty sources, max_words prompt)
- Robot unit tests for `increment_patch` helper (5 tests)
- MCP server tests for StringOrList coercion and empty list deserialization (3 tests)
- Test map table in CONTRIBUTING.md documenting all ~200 tests by area

### Changed

- All backend structs now derive `Debug` for improved error messages in tests
- Backend structs (brave, tavily, exa, serpapi, google, bing) now have a `base_url` field to support wiremock testing without modifying production URLs
- CONTRIBUTING.md feature flags table updated from 5 to 8 backends

---

## [0.1.11] - 2026-03-25

### Added

- CLI arguments for all search backends: `--searxng-url`, `--brave-api-key`, `--tavily-api-key`, `--exa-api-key`, `--serpapi-api-key` (Google and Bing already had CLI args) — enables per-instance backend configuration when running multiple IDE/agent instances
- `robot bump` now automatically updates the version badge in README.md
- `robot bump` stages README.md alongside Cargo.toml and CHANGELOG.md
- `[package.metadata.docs.rs]` in webshift Cargo.toml — builds docs.rs documentation with all features enabled

### Changed

- README.md rewritten with proper introduction: "What is WebShift" section explaining the three use cases (HTML denoiser, web content client, MCP server), "When to use / When NOT to use" guidance
- Crates.io badge now points to `webshift` crate (was `webshift-mcp`)
- Status section updated from "Beta" to "Alpha" with correct issue tracker link
- `docs/CONFIGURATION.md` backend tables now include CLI arg column for all backends
- PLAN.md workspace layout updated: added `google.rs`, `bing.rs`, `http.rs` backends and `harness.rs`; added `robot harness` command documentation; added docs.rs annotation task to M5

---

## [0.1.10] - 2026-03-25

### Added

- `AdaptiveBudget` enum with three modes: `"auto"` (default), `"on"`, `"off"` — replaces the previous boolean `adaptive_budget` flag
- Auto mode uses a dominance ratio heuristic (threshold 1.5) to decide at runtime whether proportional budget allocation would make a meaningful difference
- Custom serde deserializer for `AdaptiveBudget` accepts both TOML booleans (`true`/`false`) and strings (`"auto"`/`"on"`/`"off"`) for backward compatibility
- Harness report displays the resolved auto decision with the dominance ratio value (e.g. `auto (→ on, dominance 1.85)`)
- `docs/CONFIGURATION.md` — complete configuration reference covering all TOML keys, environment variables, and CLI arguments with plain-language descriptions
- `docs/USE_CASES.md` — 10 real-world configuration examples (local-only, cloud, tight budget, research mode, custom HTTP backend, domain filtering, etc.)
- `docs/UNDER_THE_HOOD.md` — detailed pipeline documentation with BM25 formulas, denoising stages, compression chain, and real harness output examples
- Compression percentage in harness output now shows one decimal (e.g. `99.3%` instead of `99%`)

### Changed

- Default value of `server.adaptive_budget` changed from `false` to `"auto"`
- `WEBSHIFT_ADAPTIVE_BUDGET` environment variable now accepts `"auto"`, `"on"`, `"off"` (in addition to `"true"`/`"false"` for backward compatibility)
- `README.md` restructured with links to the three new documentation pages
- Flat truncation in `lib.rs` now uses multibyte-safe `.chars().take(N)` instead of byte-slice truncation

---

## [0.1.9] - 2026-03-25

### Added

- `ServerConfig.language` — BCP-47 language tag (default `"en"`) passed to all search backends as fallback when no per-call language override is provided; prevents foreign-language results from SearXNG and similar backends
- `Stats.raw_bytes` — total raw HTML bytes downloaded before cleaning, used for compression-ratio reporting
- `robot harness`: COMPRESSION block showing raw download → clean text → LLM summary sizes (KB) with percentage reduction at each stage
- `robot harness`: `box_header()` helper with Unicode box-drawing characters for visual section separation
- `robot harness`: CONTENT PREVIEWS block with per-source `TITLE:` / `PREVIEW:` fields at column 1
- `robot harness`: full `#[ignore]` integration tests for Google, Bing, and HTTP backends in `crates/webshift/tests/`

### Changed

- `robot harness` output restructured: snippet pool → content previews → LLM summary → consolidated report at the bottom
- `robot harness` SOURCES table: removed title/url columns (correlation via `[id]` citation in content previews); all rows start at column 1
- `robot harness` banner changed to uppercase `WEBSHIFT HARNESS REPORT`
- Progress bars use Unicode block characters (`█`/`░`) instead of `#`
- `fetch_timing` map is now fully propagated (both primary and gap-fill fetches) so `raw_bytes` includes all downloads
- `language` in `lib.rs` resolved with config fallback: per-call `lang` overrides `ServerConfig.language`; empty string disables the hint
- `README.md`: documented LLM cross-language normalization as a bonus feature (foreign-language pages summarized in prompt language)

### Fixed

- `reranker.rs`: panic on multibyte UTF-8 content (e.g. Chinese) when slicing `content[..3000]` at a byte boundary; replaced with `.chars().take(3000).collect()`

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
- `TestConfig` struct with `to_webshift_config()` conversion (shared by tests and harness)
- Live integration tests: 5 backend tests, 3 LLM pipeline tests, 1 fetch test (all `#[ignore]`, require `test.toml`)
- `robot harness` subcommand — full pipeline runner with BM25 scores, budget stats, timing, consolidated report
- Example configs: `webshift.toml`, `webshift-ollama.toml`, `webshift-minimal.toml`, `webshift-brave.toml`
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
- `webshift-mcp` now builds with `llm` feature enabled
- CLI args for all LLM settings: `--llm-enabled`, `--llm-model`, `--llm-base-url`, `--llm-api-key`, `--llm-timeout`, `--llm-expansion-enabled`, `--llm-summarization-enabled`, `--llm-rerank-enabled`, `--llm-max-summary-words`, `--llm-input-budget-factor`
- Tests: `LlmClient` (4), expander (4), summarizer (2), `rerank_llm` (2), LLM pipeline (3), MCP LLM CLI args (1)

---

## [0.1.5] - 2026-03-25

### Added

- `SearchBackend` trait + `create_backend` factory with 5 implementations: SearXNG, Brave, Tavily, Exa, SerpAPI
- BM25 deterministic reranking + adaptive budget redistribution
- Full query pipeline: search → dedup → fetch → clean → rerank → assemble with oversampling and gap filler
- `webshift::query()` and `webshift::query_with_options()` public library API
- `webshift_query` MCP tool with `StringOrList` queries param, backend override, and lang support
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

- `webshift_fetch` MCP tool via `rmcp` 1.x with stdio transport
- `webshift_onboarding` MCP tool — operational guide JSON (matches Python implementation)
- CLI argument parsing with clap: `--config`, `--default-backend`, `--debug`, `--trace`, `--log-file`
- MCP server instructions for AI agent guidance
- `tracing-subscriber` logging to stderr or file

---

## [0.1.2] - 2026-03-25

### Added

- TOML config loading + `WEBSHIFT_*` env var overrides
- Cleaner test suite: port of full Python test cases (12 tests)
- Fetcher tests: wiremock retry — 429, 503, exhausted retries, 404 no-retry (6 tests)
- Config tests: TOML parsing, env override, CLI override

---

## [0.1.1] - 2026-03-25

### Added

- Workspace scaffold: `webshift` (lib), `webshift-mcp` (bin), `robot` (dev tool)
- `robot` commands: `bump`, `test`, `promote`, `unpromote`, `publish`
- HTML cleaner with `scraper`/html5ever + text sterilization pipeline
- `reqwest` concurrent fetcher with streaming cap, UA rotation, and retry
- URL utils: sanitize, dedup, binary extension filter
- `webshift::clean()` and `webshift::fetch()` public API
- Release profile with LTO, strip, and size optimization
- `CLAUDE.md`, `CONTRIBUTING.md`, `PLAN.md`

[Unreleased]: https://github.com/annibale-x/webshift/compare/v0.1.8...HEAD
[0.1.8]: https://github.com/annibale-x/webshift/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/annibale-x/webshift/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/annibale-x/webshift/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/annibale-x/webshift/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/annibale-x/webshift/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/annibale-x/webshift/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/annibale-x/webshift/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/annibale-x/webshift/releases/tag/v0.1.1
