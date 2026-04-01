# Task Completion Checklist

When a coding task is completed:
1. Run `cargo build` to verify compilation
2. Run `cargo test` (or `cargo test -p webgate` for lib-only changes)
3. Run `cargo clippy` for lint checks
4. If LLM features touched: `cargo test -p webgate --features llm`
5. Update PLAN.md checkboxes for completed milestone items
6. Update CHANGELOG.md if the change is user-facing
