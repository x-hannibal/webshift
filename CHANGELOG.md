# Changelog

* **2026-03-25: v0.1.5** - M3 complete — search backends + query pipeline
  * feat(backends): `SearchBackend` trait + `create_backend` factory with 5 implementations (SearXNG, Brave, Tavily, Exa, SerpAPI)
  * feat(reranker): BM25 deterministic reranking + adaptive budget redistribution
  * feat(query): full pipeline — search → dedup → fetch → clean → rerank → assemble with oversampling and gap filler
  * feat(query): `webgate::query()` and `webgate::query_with_options()` public API
  * feat(mcp): `webgate_query` tool with `StringOrList` queries param, backend override, lang support
  * test(backends): factory tests (4), SearXNG wiremock tests (4)
  * test(reranker): BM25 scoring, ranking, budget redistribution (6 tests)
  * test(pipeline): integration tests with mock search + mock pages (8 tests)
  * test(mcp): QueryParams deserialization tests (3 tests)
  * docs(plan): check off all M3 tasks

---

* **2026-03-25: v0.1.4** - M2 tests + robot auto-commit
  * test(mcp): server construction, onboarding JSON, CLI parsing, param deserialization (10 new tests)
  * feat(robot): `bump` now auto-commits all tracked changes (not just Cargo/CHANGELOG)
  * docs(plan): add M2 test task

---

* **2026-03-25: v0.1.3** - M2 complete — MCP server with fetch tool
  * feat(mcp): `webgate_fetch` tool via `rmcp` 1.x with stdio transport
  * feat(mcp): `webgate_onboarding` tool — operational guide JSON (matches Python)
  * feat(mcp): CLI argument parsing with clap (`--config`, `--default-backend`, `--debug`, `--trace`, `--log-file`)
  * feat(mcp): server instructions for AI agent guidance
  * feat(mcp): tracing-subscriber logging to stderr or file
  * docs(plan): check off all M2 tasks

---

* **2026-03-25: v0.1.2** - M1 complete — config, tests
  * feat(config): TOML loading + `WEBGATE_*` env var overrides + tests
  * test(cleaner): port full Python test suite (12 new tests)
  * test(fetcher): wiremock retry tests — 429, 503, exhausted retries, 404 no-retry (6 new tests)
  * docs(plan): check off all M1 tasks

---

* **2026-03-25: v0.1.1** - Initial workspace scaffold
  * feat(workspace): setup `webgate` (lib), `webgate-mcp` (bin), `robot` (dev tool)
  * feat(robot): `bump`, `test`, `promote`, `unpromote`, `publish` commands
  * feat(cleaner): HTML cleaning with `scraper`/html5ever + text sterilization pipeline
  * feat(fetcher): reqwest concurrent fetcher with streaming cap, UA rotation, retry
  * feat(url): sanitize, dedup, binary extension filter
  * feat(lib): `webgate::clean()` and `webgate::fetch()` public API (initial scaffold)
  * chore(build): release profile with LTO, strip, size optimization
  * docs: CLAUDE.md, CONTRIBUTING.md, PLAN.md

---
