# Suggested Commands

## Build & Test
```bash
cargo build                              # build all crates
cargo test                               # run full test suite
cargo test -p webgate                    # test library only
cargo test -p webgate -- test_name       # run single test
cargo test -p webgate --features llm     # test with LLM features
```

## Run
```bash
cargo run -p webgate-mcp                 # start MCP server (stdio)
```

## Release (robot tool)
```bash
cargo run -p robot -- bump               # auto-increment patch version
cargo run -p robot -- bump 1.2.3         # bump to specific version
cargo run -p robot -- test               # cargo test all crates
cargo run -p robot -- promote            # build+test+merge dev→main+tag+push
cargo run -p robot -- unpromote          # undo last promote
cargo run -p robot -- publish            # cargo publish to crates.io
```

## System Utils (Windows/bash)
```bash
git status / git log / git diff          # version control
ls / dir                                 # list files
```
