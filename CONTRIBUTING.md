# Contributing

## Branch model

All development happens on `dev`. The `main` branch contains only released,
tagged commits. Never commit directly to `main`.

```
main    ← release tags only (v0.1.0, v0.2.0, …)
dev     ← active development
```

Feature branches are optional for larger isolated work:

```bash
git checkout -b feat/my-feature   # branch off dev
# … work …
git checkout dev
git merge feat/my-feature --no-ff
git branch -d feat/my-feature
```

---

## Development setup

```bash
# No system packages required — pure Rust
cargo build          # build all crates
cargo test           # run full test suite
cargo test -p webgate               # library only
cargo test -p webgate -- test_name  # single test
cargo run -p webgate-mcp            # start MCP server
```

---

## Release workflow

The `robot` crate automates versioning and promotion.
Run all `robot` commands from the workspace root.

```bash
cargo run -p robot -- <command> [args]
```

### Step 1 — Update CHANGELOG

Before any bump, update `CHANGELOG.md` manually (or ask Claude to do it).
Follow the [Keep a Changelog](https://keepachangelog.com) format:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- …

### Changed
- …

### Fixed
- …

### Removed
- …
```

### Step 2 — Bump version

```bash
cargo run -p robot -- bump          # auto-increment patch (Z+1)
cargo run -p robot -- bump 0.2.0    # explicit version
```

What `bump` does:
1. Updates `[workspace.package] version` in root `Cargo.toml`.
2. Commits `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` with message
   `chore(release): bump to X.Y.Z`.

All crates share the workspace version — a single bump covers everything.

### Run tests

```bash
cargo run -p robot -- test
```

Runs `cargo test --workspace`. Use this to verify everything passes before promoting.

### Step 3 — Promote

```bash
cargo run -p robot -- promote
```

What `promote` does:
1. `cargo build --release` — aborts on failure.
2. `cargo test` — aborts on failure.
3. Merges `dev` → `main` with a no-ff merge commit `release: vX.Y.Z`.
4. Creates tag `vX.Y.Z`.
5. Pushes `main` and the tag to `origin`.
6. Checks out `dev`.

### Step 4 — Publish (M5+)

```bash
cargo run -p robot -- publish
```

Publishes `webgate` then `webgate-mcp` to crates.io in order.
Only run after a successful `promote`.

### Undo a promote

```bash
cargo run -p robot -- unpromote
```

Deletes the remote and local tag, resets `main` to the previous commit
(`--force-with-lease`), and checks out `dev`. Use immediately after a bad promote.

---

## Commit convention

Format: `type(scope): description`

| Type | When |
|------|------|
| `feat` | new functionality |
| `fix` | bug fix |
| `refactor` | restructure without behavior change |
| `test` | add or fix tests |
| `docs` | documentation only |
| `chore` | tooling, deps, CI |
| `style` | formatting, no logic change |

**Do not add `Co-Authored-By` tags.**

Examples:
```
feat(cleaner): port html5ever noise-removal pipeline
fix(fetcher): respect Retry-After header on 429
chore(deps): add scraper 0.20 with html5ever
```

---

## Crate structure

| Crate | Published | Role |
|-------|-----------|------|
| `webgate` | yes | Library: `clean()`, `fetch()`, `query()` |
| `webgate-mcp` | yes | Binary: MCP server (`mcp-webgate`) |
| `robot` | no | Dev tool: bump / test / promote / unpromote / publish |

### Feature flags (`webgate`)

| Feature | Default | Enables |
|---------|---------|---------|
| `backends` | on | All 8 search backends + query pipeline |
| `llm` | off | LLM client, query expander, summarizer, LLM reranking |

Depend only on the cleaner + fetcher:
```toml
webgate = { version = "x.y.z", default-features = false }
```

---

## Testing

### Test map

| Area | File | Tests | What's covered |
|------|------|------:|----------------|
| **Cleaner** | `scraper/cleaner.rs` | 17 | HTML noise removal, text sterilization, typography normalization, apply_window budget, process_page pipeline, title extraction |
| **Fetcher** | `scraper/fetcher.rs` | 9 | Streaming fetch, size cap, UA rotation, retry on 429/503, concurrent fetch, 404 handling |
| **URL utils** | `utils/url.rs` | 8 | Sanitize tracking params, binary extension filter, domain allow/blocklist, dedup |
| **Reranker** | `utils/reranker.rs` | 13 | BM25 scoring, deterministic rerank, budget redistribution, LLM rerank + fallback, edge cases (empty input, no surplus) |
| **Config** | `config.rs` | 14 | Defaults, TOML parsing, env var override, AdaptiveBudget deserialization (bool/string), env_bool variants |
| **SearXNG** | `backends/searxng.rs` | 4 | Result parsing, empty results, HTTP error, lang param |
| **Brave** | `backends/brave.rs` | 8 | Result parsing, safesearch mapping, missing web key, num_results cap, lang param |
| **Tavily** | `backends/tavily.rs` | 6 | POST body, result parsing, num_results cap (20), lang ignored |
| **Exa** | `backends/exa.rs` | 7 | Highlights-first snippet, text fallback, num_results cap (10), lang ignored |
| **SerpAPI** | `backends/serpapi.rs` | 5 | organic_results parsing, link→url mapping, lang→hl override |
| **Google** | `backends/google.rs` | 7 | Dual validation (api_key + cx), items parsing, lr=lang_xx, cap at 10, missing items key |
| **Bing** | `backends/bing.rs` | 7 | webPages.value parsing, name→title, market from lang, cap at 50, missing webPages key |
| **HTTP** | `backends/http.rs` | 13 | GET/POST, json_path traversal, custom field names, headers, extra_params, lang_param, count_param omission |
| **Backend factory** | `backends/mod.rs` | 7 | Factory by name, validation errors for all backends |
| **LLM client** | `llm/client.rs` | 7 | Chat response, disabled error, HTTP error, auth header, empty choices, invalid JSON, no-key header |
| **LLM expander** | `llm/expander.rs` | 6 | Expansion + variants, n=1 skip, fallback on error, markdown fence strip, non-array fallback, n cap |
| **LLM summarizer** | `llm/summarizer.rs` | 4 | Markdown output, error propagation, empty sources, max_words in prompt |
| **Pipeline** | `lib.rs` | 17 | Search→fetch→clean→rerank, dedup, binary filter, snippet pool, multi-query round-robin, LLM expansion, summarization, error capture |
| **Public API** | `lib.rs` | 5 | `clean()` fields/truncation/noise, `fetch()` binary/domain filters + fields, `query()` empty input |
| **MCP server** | `webgate-mcp` | 17 | CLI parsing (all args), tool params deserialization, onboarding JSON, server info, StringOrList coercion |
| **Robot** | `robot` | 5 | increment_patch (basic, zero, large, invalid format, non-numeric) |
| | | **~200** | |

### Unit tests (mocked)

All unit and pipeline tests use `wiremock` mock servers — no external services needed:

```bash
cargo test --workspace                          # full suite
cargo test -p webgate --features llm            # include LLM tests
```

### Integration tests (real services)

Integration tests hit real backends and LLM services configured in `test.toml`.
They are marked `#[ignore]` and skipped by default.

**Setup:**

```bash
cp test.toml.example test.toml   # copy template
# edit test.toml — enable backends you have, add API keys
```

**Run:**

```bash
cargo test -p webgate --features llm -- --ignored          # all integration tests
cargo test -p webgate -- --ignored searxng_live             # single backend
cargo test -p webgate --features llm -- --ignored llm_      # LLM tests only
```

Each test checks its backend/LLM `enabled` flag and prints `SKIP` if disabled.

### Diagnostic harness

The `robot harness` subcommand runs the full pipeline against real services
with verbose diagnostic output — useful for tuning BM25, budget allocation,
and reranking parameters.

```bash
cargo run -p robot -- harness "rust async programming"
cargo run -p robot -- harness "quantum computing" --backend brave -n 10
```

Output includes: pipeline stats, per-source details, BM25 score distribution,
adaptive budget allocation, snippet pool, and LLM summary.
