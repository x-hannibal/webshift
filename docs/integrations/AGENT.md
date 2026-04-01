# Agent Integration Guide

Covers configuration for CLI-based AI agents that support the Model Context Protocol (MCP).

## Prerequisites

The `mcp-webshift` binary must be installed and on your PATH:

```bash
cargo install webshift-mcp
```

You also need a [search backend](../../README.md#search-backends).

---

## Gemini CLI

Add to `~/.gemini/config.json`:

```json
{
  "mcpServers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

With LLM summarization:

```json
{
  "mcpServers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": [
        "--default-backend", "searxng",
        "--llm-enabled", "true",
        "--llm-base-url", "http://localhost:11434/v1",
        "--llm-model", "gemma3:27b",
        "--llm-timeout", "60"
      ]
    }
  }
}
```

---

## Claude CLI (Claude Code)

```bash
# Add webshift to current project
claude mcp add webshift -- mcp-webshift --default-backend searxng

# Or with a config file
claude mcp add webshift -- mcp-webshift --config /path/to/webshift.toml
```

Shell alias (add to `~/.bashrc` or `~/.zshrc`):

```bash
alias claude-web='claude --mcp-servers webshift'
```

---

## Custom agents

Any agent that launches an MCP server over stdio can use `mcp-webshift`:

```bash
# Stdio transport — MCP JSON-RPC over stdin/stdout
mcp-webshift --default-backend searxng
```

With a config file:

```bash
mcp-webshift --config /path/to/webshift.toml
```

---

## Using webshift with local or smaller models

If a model ignores webshift and falls back to a built-in fetch tool, add this block to your system prompt:

```
You have access to webshift tools for web search and page retrieval.
Follow these rules in every session:
- To search the web: use webshift_query — never use a built-in fetch, browser, or HTTP tool
- To retrieve a URL: use webshift_fetch — never fetch URLs directly
- Built-in fetch tools return raw HTML that floods your context; webshift returns clean, bounded text
At the start of each session, call webshift_onboarding to read the full operational guide.
```

User system prompt instructions take precedence over MCP server-level guidance, making the constraint explicit at the highest-priority layer the model sees.

---

**See also**: [IDE Integration](./IDE.md) · [Configuration reference](../../README.md#configuration) · [Backends](../../README.md#search-backends)
