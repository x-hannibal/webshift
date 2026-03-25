# IDE Integration Guide

Covers configuration for desktop AI clients and IDEs that support the Model Context Protocol (MCP).

## Prerequisites

The `mcp-webgate` binary must be installed and on your PATH:

```bash
cargo install webgate-mcp
```

Or from source:

```bash
cargo install --path crates/webgate-mcp
```

You also need a [search backend](../../README.md#search-backends). The easiest option is SearXNG:

```bash
docker run -d -p 4000:8080 searxng/searxng
```

No Docker? Use a cloud backend — see [Backends](../../README.md#search-backends) for Brave, Tavily, Exa, SerpAPI, Google, or Bing.

---

## Claude Desktop

Open the config file:

- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Linux**: `~/.config/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

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

With a cloud backend:

```json
{
  "mcpServers": {
    "webgate": {
      "command": "mcp-webgate",
      "args": ["--default-backend", "brave", "--brave-api-key", "BSA..."]
    }
  }
}
```

With LLM summarization (Ollama):

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

**After editing**: completely quit and restart Claude Desktop.

**Troubleshooting — "spawn mcp-webgate ENOENT"**: Claude Desktop has a restricted PATH. Use the full binary path:

```bash
which mcp-webgate   # macOS/Linux
where mcp-webgate   # Windows
```

```json
{ "command": "/home/yourname/.cargo/bin/mcp-webgate" }
```

---

## Claude Code

Create `.mcp.json` in your project folder (or `~/.mcp.json` for global config):

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

Or add via CLI:

```bash
claude mcp add webgate -- mcp-webgate --default-backend searxng
```

---

## Zed Editor

Add to `~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "webgate": {
      "command": {
        "path": "mcp-webgate",
        "args": ["--default-backend", "searxng"]
      }
    }
  }
}
```

> Zed uses `"context_servers"` (not `"mcpServers"`) and requires a nested `"command"` object.

---

## Cursor

Create `.cursor/mcp.json` in your project root (or `~/.cursor/mcp.json` for global):

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

> MCP tools in Cursor only work in **Agent mode**.

---

## Windsurf

Edit `~/.codeium/windsurf/mcp_config.json` (Windows: `C:\Users\<user>\.codeium\windsurf\mcp_config.json`):

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

> Windsurf only supports global MCP configuration (no per-project config).

---

## VS Code

Create `.vscode/mcp.json` in your workspace root:

```json
{
  "servers": {
    "webgate": {
      "command": "mcp-webgate",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

> VS Code uses `"servers"` (not `"mcpServers"`). MCP tools work in **Agent mode** only (Copilot Chat).

---

## Multi-instance setup

Running webgate in multiple IDEs simultaneously? Use CLI args — each instance gets independent config and integers stay integers (no string-wrapping):

```json
{
  "mcpServers": {
    "webgate": {
      "command": "mcp-webgate",
      "args": [
        "--default-backend", "searxng",
        "--llm-enabled",
        "--llm-model", "gemma3:27b",
        "--llm-timeout", "60"
      ]
    }
  }
}
```

Precedence: `CLI args > env vars > webgate.toml > defaults`

Full reference: `mcp-webgate --help`

---

## Config file locations

| Client | Config path |
|--------|-------------|
| Claude Desktop (macOS) | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Claude Desktop (Linux) | `~/.config/Claude/claude_desktop_config.json` |
| Claude Desktop (Windows) | `%APPDATA%\Claude\claude_desktop_config.json` |
| Claude Code | `.mcp.json` in project root (or `~/.mcp.json` global) |
| Zed | `~/.config/zed/settings.json` |
| Cursor (project) | `.cursor/mcp.json` |
| Cursor (global) | `~/.cursor/mcp.json` |
| Windsurf (macOS/Linux) | `~/.codeium/windsurf/mcp_config.json` |
| Windsurf (Windows) | `C:\Users\<user>\.codeium\windsurf\mcp_config.json` |
| VS Code (workspace) | `.vscode/mcp.json` |

---

**See also**: [Agent Integration](./AGENT.md) · [Configuration reference](../../README.md#configuration) · [Backends](../../README.md#search-backends)
