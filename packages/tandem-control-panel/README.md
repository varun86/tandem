# Tandem Control Panel

Full web control center for Tandem Engine (non-desktop entry point).

## Install

```bash
npm i -g @frumu/tandem-panel
```

## Run

```bash
tandem-control-panel
```

Alias also supported:

```bash
tandem-setup
```

Bootstrap env/token first (recommended):

```bash
tandem-control-panel --init
```

Or:

```bash
tandem-control-panel-init
```

## Features

- Token-gated web portal
- Dashboard + health overview
- Chat + session management
- Routines/automations
- Channels (Telegram/Discord/Slack)
- MCP server management
- Node-based swarm orchestration + live flow visualization
- Memory browsing/search/delete
- Agent teams + mission approvals
- Global live event feed
- Provider settings

## Environment

Copy and customize if needed:

```bash
cp .env.example .env
```

Variables:

- `TANDEM_CONTROL_PANEL_PORT` (default `39732`)
- `TANDEM_ENGINE_URL` (default `http://127.0.0.1:39731`)
- `TANDEM_ENGINE_HOST` + `TANDEM_ENGINE_PORT` fallback
- `TANDEM_CONTROL_PANEL_AUTO_START_ENGINE` (`1`/`0`)
- `TANDEM_CONTROL_PANEL_ENGINE_TOKEN` (token injected when panel auto-starts engine)
- `TANDEM_API_TOKEN` (backward-compatible alias for engine token)
- `TANDEM_CONTROL_PANEL_SESSION_TTL_MINUTES` (default `1440`)

## Token Behavior

- If the panel connects to an already-running engine, use that engine's API token to sign in.
- If the panel auto-starts an engine, it now always starts with a known token:
  - `TANDEM_CONTROL_PANEL_ENGINE_TOKEN` (preferred), or
  - `TANDEM_API_TOKEN` (alias), or
  - auto-generated at startup (printed in panel logs).

## Setup Flow

1. Run `tandem-control-panel --init` to create/update `.env` and generate a token if missing.
2. Run `tandem-control-panel`.
3. Sign in with the printed `TANDEM_CONTROL_PANEL_ENGINE_TOKEN`.

## Development

```bash
cd packages/tandem-control-panel
npm install
npm run dev
npm run build
```
