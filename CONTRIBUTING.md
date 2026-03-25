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
| `backends` | on | All 5 search backends + query pipeline |
| `llm` | off | LLM client, query expander, summarizer, LLM reranking |

Depend only on the cleaner + fetcher:
```toml
webgate = { version = "x.y.z", default-features = false }
```
