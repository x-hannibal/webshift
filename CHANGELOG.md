# Changelog

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
