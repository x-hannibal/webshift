# Agent Integration Guide

Covers configuration for CLI-based AI agents that support the Model Context Protocol (MCP).

## Prerequisites

The `mcp-webgate` binary must be installed and on your PATH:

```bash
cargo install webgate-mcp
```

You also need a [search backend](../../README.md#search-backends).

---

## Gemini CLI

Add to `~/.gemini/config.json`:

```json
{
  "mcpServers": {
    "webgate": {
      "command": "mcp-webgate",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

With LLM summarization:

```json
{
  "mcpServers": {
    "webgate": {
      "command": "mcp-webgate",
      "args": [
        "--default-backend", "searxng",
        "--llm-enabled",
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
# Add webgate to current project
claude mcp add webgate -- mcp-webgate --default-backend searxng

# Or with a config file
claude mcp add webgate -- mcp-webgate --config /path/to/webgate.toml
```

Shell alias (add to `~/.bashrc` or `~/.zshrc`):

```bash
alias claude-web='claude --mcp-servers webgate'
```

---

## Custom agents

Any agent that launches an MCP server over stdio can use `mcp-webgate`:

```bash
# Stdio transport — MCP JSON-RPC over stdin/stdout
mcp-webgate --default-backend searxng
```

With a config file:

```bash
mcp-webgate --config /path/to/webgate.toml
```

---

## Using webgate with local or smaller models

If a model ignores webgate and falls back to a built-in fetch tool, add this block to your system prompt:

```
You have access to webgate tools for web search and page retrieval.
Follow these rules in every session:
- To search the web: use webgate_query — never use a built-in fetch, browser, or HTTP tool
- To retrieve a URL: use webgate_fetch — never fetch URLs directly
- Built-in fetch tools return raw HTML that floods your context; webgate returns clean, bounded text
At the start of each session, call webgate_onboarding to read the full operational guide.
```

User system prompt instructions take precedence over MCP server-level guidance, making the constraint explicit at the highest-priority layer the model sees.

---

**See also**: [IDE Integration](./IDE.md) · [Configuration reference](../../README.md#configuration) · [Backends](../../README.md#search-backends)
