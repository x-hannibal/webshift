# IDE Integration Guide

Covers configuration for desktop AI clients and IDEs that support the Model Context Protocol (MCP).

## Prerequisites

The `mcp-webshift` binary must be installed and on your PATH:

```bash
cargo install webshift-mcp
```

Or from source:

```bash
cargo install --path crates/webshift-mcp
```

You also need a [search backend](../../README.md#search-backends). The easiest option is SearXNG:

```bash
docker run -d -p 8080:8080 searxng/searxng
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
    "webshift": {
      "command": "mcp-webshift",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

With a cloud backend:

```json
{
  "mcpServers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": ["--default-backend", "brave", "--brave-api-key", "BSA..."]
    }
  }
}
```

With LLM summarization (Ollama):

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

**After editing**: completely quit and restart Claude Desktop.

**Troubleshooting — "spawn mcp-webshift ENOENT"**: Claude Desktop has a restricted PATH. Use the full binary path:

```bash
which mcp-webshift   # macOS/Linux
where mcp-webshift   # Windows
```

```json
{ "command": "/home/yourname/.cargo/bin/mcp-webshift" }
```

---

## Claude Code

Create `.mcp.json` in your project folder (or `~/.mcp.json` for global config):

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

Or add via CLI:

```bash
claude mcp add webshift -- mcp-webshift --default-backend searxng
```

---

## Zed Editor

### Option A — Extension (recommended)

Install the **mcp-webshift** extension from the Zed extension marketplace.
The extension downloads the native binary automatically and exposes all settings
through a **Configure Server** modal — no manual file editing required.

Full guide: [Zed Extension](./ZED_EXTENSION.md)

### Option B — Manual config

If you prefer direct control, add to `~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "mcp-webshift": {
      "command": {
        "path": "mcp-webshift",
        "args": ["--default-backend", "searxng"]
      }
    }
  }
}
```

The binary must be on your PATH (`cargo install webshift-mcp`).

---

## Cursor

Create `.cursor/mcp.json` in your project root (or `~/.cursor/mcp.json` for global):

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

> MCP tools in Cursor only work in **Agent mode**.

---

## Windsurf

Edit `~/.codeium/windsurf/mcp_config.json` (Windows: `C:\Users\<user>\.codeium\windsurf\mcp_config.json`):

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

> Windsurf only supports global MCP configuration (no per-project config).

---

## VS Code

Create `.vscode/mcp.json` in your workspace root:

```json
{
  "servers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

> VS Code uses `"servers"` (not `"mcpServers"`). MCP tools work in **Agent mode** only (Copilot Chat).

---

## Multi-instance setup

Running webshift in multiple IDEs simultaneously? Use CLI args — each instance gets independent config and integers stay integers (no string-wrapping):

```json
{
  "mcpServers": {
    "webshift": {
      "command": "mcp-webshift",
      "args": [
        "--default-backend", "searxng",
        "--llm-enabled", "true",
        "--llm-model", "gemma3:27b",
        "--llm-timeout", "60"
      ]
    }
  }
}
```

Precedence: `CLI args > env vars > webshift.toml > defaults`

Full reference: `mcp-webshift --help`

---

## Config file locations

| Client | Config path |
|--------|-------------|
| Claude Desktop (macOS) | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Claude Desktop (Linux) | `~/.config/Claude/claude_desktop_config.json` |
| Claude Desktop (Windows) | `%APPDATA%\Claude\claude_desktop_config.json` |
| Claude Code | `.mcp.json` in project root (or `~/.mcp.json` global) |
| Zed (extension) | Zed Extension marketplace + Configure Server modal |
| Zed (manual) | `~/.config/zed/settings.json` |
| Cursor (project) | `.cursor/mcp.json` |
| Cursor (global) | `~/.cursor/mcp.json` |
| Windsurf (macOS/Linux) | `~/.codeium/windsurf/mcp_config.json` |
| Windsurf (Windows) | `C:\Users\<user>\.codeium\windsurf\mcp_config.json` |
| VS Code (workspace) | `.vscode/mcp.json` |

---

**See also**: [Agent Integration](./AGENT.md) · [Configuration reference](../../README.md#configuration) · [Backends](../../README.md#search-backends)
