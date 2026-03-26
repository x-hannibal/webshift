# Zed Extension

The `mcp-webshift` Zed extension installs and manages the native binary automatically.
No manual PATH setup, no config file editing — everything is handled through Zed's
**Configure Server** modal.

---

## Installation

1. Open Zed → **Extensions** (`Cmd+Shift+X` / `Ctrl+Shift+X`)
2. Search for **mcp-webshift**
3. Click **Install**

The first time the context server starts, the extension downloads the correct
native binary for your platform from GitHub Releases. No runtime required.

---

## Configuration

Right-click **mcp-webshift** in the **Context Servers** panel → **Configure Server**.

The JSON keys are CLI flags — the same flags accepted by `mcp-webshift --help`.
Full reference: [CLI arguments](../CONFIGURATION.md#cli-arguments-mcp-server-only).

### Examples

**SearXNG (self-hosted, no API key)**

```json
{
  "--default-backend": "searxng",
  "--searxng-url": "http://localhost:8080"
}
```

**Brave**

```json
{
  "--default-backend": "brave",
  "--brave-api-key": "BSA-..."
}
```

**With LLM summarization (Ollama)**

```json
{
  "--default-backend": "searxng",
  "--searxng-url": "http://localhost:8080",
  "--llm-enabled": true,
  "--llm-base-url": "http://localhost:11434/v1",
  "--llm-model": "gemma3:27b"
}
```

---

## Alternative: manual setup (without the extension)

If you prefer not to use the extension, add the server directly to
`~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "mcp-webshift": {
      "command": "mcp-webshift",
      "args": ["--default-backend", "searxng"]
    }
  }
}
```

The binary must be on your PATH (`cargo install webshift-mcp`).

---

**See also**: [IDE Integration](./IDE.md) · [Configuration reference](../CONFIGURATION.md) · [Backends](../../README.md#search-backends)
