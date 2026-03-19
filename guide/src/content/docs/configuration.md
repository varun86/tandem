---
title: Configuration
---

Tandem Engine uses a layered configuration system that prioritizes settings from different sources. This allows for global defaults, per-project overrides, and environment-based secrets.

## Configuration Layers

Settings are resolved in the following order (highest priority first):

1. **Environment Variables**: Secrets and explicit overrides.
2. **Managed Config**: `managed_config.json` (for automated/managed environments).
3. **Project Config**: `.tandem/config.json` in your workspace.
4. **Global Config**: `~/.config/tandem/config.json` (Linux/Mac) or `%APPDATA%\tandem\config.json` (Windows).

## Environment Variables

### Provider Keys

Tandem automatically maps standard API key variables to their respective providers:

- `OPENAI_API_KEY` → `openai`
- `ANTHROPIC_API_KEY` → `anthropic`
- `OPENROUTER_API_KEY` → `openrouter`
- `GROQ_API_KEY` → `groq`
- `MISTRAL_API_KEY` → `mistral`
- `TOGETHER_API_KEY` → `together`
- `COHERE_API_KEY` → `cohere`
- `GITHUB_TOKEN` → `copilot`
- `AZURE_OPENAI_API_KEY` → `azure`
- `VERTEX_API_KEY` → `vertex`
- `BEDROCK_API_KEY` → `bedrock`

### Ollama

- `OLLAMA_URL`: Overrides the default Ollama URL (default: `http://127.0.0.1:11434/v1`).

### System paths

- `AGENT_GLOBAL_CONFIG`: Canonical override path to the global configuration file.
- `TANDEM_GLOBAL_CONFIG`: Backward-compatible alias for global config path.
- `TANDEM_STATE_DIR`: Override the directory where the engine stores its state (logs, database, etc.).

### Identity and personality

- `AGENT_BOT_NAME`: Canonical assistant name override.
- `AGENT_PERSONA`: Canonical custom personality instruction override.
- `TANDEM_BOT_NAME`: Backward-compatible alias for bot name.
- `TANDEM_PERSONA`: Backward-compatible alias for personality.

Use the identity API for structured settings (bot aliases, personality presets, per-agent overrides):

- `GET /config/identity`
- `PATCH /config/identity`

### Protocol branding

- `AGENT_PROTOCOL_TITLE`: Canonical protocol/application title override used in provider metadata (for example OpenRouter `X-Title`).
- `TANDEM_PROTOCOL_TITLE`: Backward-compatible alias for protocol title.

### Automation cost estimation

- `TANDEM_TOKEN_COST_PER_1K_USD`: Estimated USD cost per 1,000 tokens used by automation/routine runs.
  - Used by dashboard **Automations + Cost** metrics.
  - Default: `0` (cost tracking disabled unless explicitly configured).

### Built-in web search

- `TANDEM_SEARCH_BACKEND`: Selects the built-in `websearch` backend.
  - Supported values: `tandem`, `brave`, `exa`, `searxng`, `none`
  - Official installs default to `tandem`.
- `TANDEM_SEARCH_URL`: Hosted Tandem search endpoint or compatible router URL used when `TANDEM_SEARCH_BACKEND=tandem`.
- `TANDEM_SEARCH_TIMEOUT_MS`: Request timeout for built-in web search.
- `TANDEM_BRAVE_SEARCH_API_KEY`: Direct Brave Search API key when `TANDEM_SEARCH_BACKEND=brave`.
- `TANDEM_EXA_API_KEY`: Direct Exa API key when `TANDEM_SEARCH_BACKEND=exa`.
- `TANDEM_SEARXNG_URL`: Self-hosted SearXNG endpoint when `TANDEM_SEARCH_BACKEND=searxng`.

## Config File Format

The configuration file is a simple JSON object.

```json
{
  "default_provider": "anthropic",
  "providers": {
    "anthropic": {
      "default_model": "claude-3-5-sonnet-latest"
    },
    "openai": {
      "default_model": "gpt-4o"
    },
    "ollama": {
      "url": "http://localhost:11434/v1",
      "default_model": "llama3"
    }
  }
}
```

## Setup Wizard

When you first run the Tandem TUI, if no providers are configured, it will launch a **Setup Wizard** to help you configure your `default_provider` and model. This configuration is saved to your global config file.
