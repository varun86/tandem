# Tandem Master CLI and Engine Wrapper

```text
TTTTT   A   N   N DDDD  EEEEE M   M
  T    A A  NN  N D   D E     MM MM
  T   AAAAA N N N D   D EEEE  M M M
  T   A   A N  NN D   D E     M   M
  T   A   A N   N DDDD  EEEEE M   M
```

## What This Is

Prebuilt npm distribution of Tandem for macOS, Linux, and Windows.

Installing this package gives you the master `tandem` CLI plus the direct
`tandem-engine` runtime binary without compiling Rust locally.

If you want to build from Rust source instead, use the crate docs in `engine/README.md`.

## Install

```bash
npm install -g @frumu/tandem
```

The installer downloads the release asset that matches this package version. Tags and package versions are expected to match (for example, `v0.4.16`).

## Quick Start

Inspect the installation:

```bash
tandem doctor
```

Install the optional web control panel add-on:

```bash
tandem install panel
tandem panel init
```

Open the panel once it is installed:

```bash
tandem panel open
```

Start the engine server directly when you want a foreground runtime:

```bash
tandem-engine serve --hostname 127.0.0.1 --port 39731
```

## Commands

### Master CLI

```bash
tandem doctor
tandem status
tandem service install
tandem service status
tandem service restart
tandem update
```

Panel and add-on management:

```bash
tandem install panel
tandem panel status
tandem panel open
tandem addon list
```

### Serve

```bash
tandem-engine serve --hostname 127.0.0.1 --port 39731
```

Options include:

- `--hostname` or `--host`
- `--port`
- `--state-dir`
- `--provider`
- `--model`
- `--api-key`
- `--config`
- `--api-token`

### Run a Single Prompt

```bash
tandem-engine run "What is the capital of France?"
```

### Run a Concurrent Batch

```bash
cat > tasks.json << 'JSON'
{
  "tasks": [
    { "id": "plan", "prompt": "Create a 3-step rollout plan." },
    { "id": "risks", "prompt": "List top 5 rollout risks." },
    { "id": "comms", "prompt": "Draft a short launch update." }
  ]
}
JSON

tandem-engine parallel --json @tasks.json --concurrency 3
```

### Execute a Tool Directly

```bash
tandem-engine tool --json '{"tool":"workspace_list_files","args":{"path":"."}}'
```

### List Providers

```bash
tandem-engine providers
```

## Configuration

Tandem Engine merges config from:

1. Environment variables
2. `managed_config.json`
3. Project config at `.tandem/config.json`
4. Global config:
   - macOS/Linux: `~/.config/tandem/config.json`
   - Windows: `%APPDATA%\tandem\config.json`

Common provider keys:

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `OPENROUTER_API_KEY`
- `GROQ_API_KEY`
- `MISTRAL_API_KEY`
- `TOGETHER_API_KEY`
- `COHERE_API_KEY`
- `GITHUB_TOKEN` (Copilot)
- `AZURE_OPENAI_API_KEY`
- `VERTEX_API_KEY`
- `BEDROCK_API_KEY`

## Documentation

- Project docs: https://tandem.ac/docs
- GitHub releases: https://github.com/frumu-ai/tandem/releases
