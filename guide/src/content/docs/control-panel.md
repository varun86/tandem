---
title: Control Panel (Web Admin)
description: Install and run the Tandem web control panel from npm.
---

Use the control panel when you want a browser UI for chat, orchestrator, automations, memory, live feed, packs, and runtime ops.

## Install

```bash
npm i -g @frumu/tandem-panel
```

## Initialize Environment (Recommended)

```bash
tandem-control-panel --init
```

This creates/updates `.env` and ensures an engine token is available.

## Run

```bash
tandem-control-panel
```

Open:

- `http://127.0.0.1:39732`

Aliases:

- `tandem-setup`
- `tandem-control-panel-init` (init only)

## Optional: Install systemd Services (Linux)

```bash
sudo tandem-control-panel --install-services
```

Useful options:

- `--service-mode=both|engine|panel` (default `both`)
- `--service-user=<linux-user>`

## Core Environment Variables

- `TANDEM_CONTROL_PANEL_PORT` (default `39732`)
- `TANDEM_ENGINE_URL` (default `http://127.0.0.1:39731`)
- `TANDEM_CONTROL_PANEL_AUTO_START_ENGINE` (`1` or `0`)
- `TANDEM_CONTROL_PANEL_ENGINE_TOKEN` (engine API token)

## Automations + Cost (Dashboard)

The main dashboard includes a first-class **Automations + Cost** section that aggregates:

- Token usage (`24h`, `7d`) from run telemetry.
- Estimated USD cost (`24h`, `7d`).
- Top automation/routine IDs by estimated cost, token volume, and run count.

This includes legacy automations/routines and advanced multi-agent automation runs.

Cost estimation uses the engine rate:

- `TANDEM_TOKEN_COST_PER_1K_USD` (USD per 1,000 tokens, default `0`).

## Control Panel Shell

The control panel now uses a shell with:

- an icon rail for primary navigation
- a context rail for system status and actions
- a main workspace with animated route transitions and page headers

The web app intentionally pushes motion a bit further than the Tauri app while keeping the same overall visual language.

## Automations Workspace (Tabbed + Wizard)

The left nav `Automations` page (`#/automations`) now uses task-focused tabs:

- `Overview`
- `Routines`
- `Automations`
- `Templates`
- `Runs & Approvals`

A built-in walkthrough wizard can be launched from the page header and also auto-opens for first-time empty workspaces.

Legacy `#/agents` links continue to redirect for backwards compatibility.

Deep-link query state is supported on `#/automations`:

- `tab`
- `wizard`
- `flow` (`routine` or `advanced`)
- `step`

## Verify Engine + Panel

```bash
curl -s http://127.0.0.1:39731/global/health \
  -H "X-Agent-Token: tk_your_token"
```

## See Also

- [Headless Service](./headless-service/)
- [Channel Integrations](./channel-integrations/)
- [Configuration](./configuration/)
