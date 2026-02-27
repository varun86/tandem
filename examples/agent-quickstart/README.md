# Tandem Agent Quickstart

A minimal self-hosted AI agent portal. Run the setup script and you have a working agent accessible at `http://your-server-ip` in under 5 minutes.

## One-command setup (VPS / Linux server)

```bash
# Clone the repo and cd into this example
cd examples/agent-quickstart

# Run as root (or sudo) — installs engine, builds portal, creates systemd services
sudo bash setup-agent.sh
```

After setup, the script prints your portal URL and sign-in token:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ✓ Tandem Agent Quickstart is running!
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Portal URL:   http://<your-server-ip>
  Sign-in key:  <generated-token>

  Services:
    sudo systemctl status tandem-engine
    sudo systemctl status tandem-agent-portal
```

> **Prerequisites:** Node.js and npm must be installed. If not:
> ```bash
> curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo bash -
> sudo apt install -y nodejs
> ```

---

## What the setup script does

| Step | Action |
|------|--------|
| 1 | Detects Node / npm (handles nvm, system, and PATH installs) |
| 2 | Installs `@frumu/tandem` globally via npm |
| 3 | Generates an API token via `tandem-engine token generate` |
| 4 | Writes `/etc/tandem/engine.env` (token + provider key placeholders) |
| 5 | Bootstraps `/srv/tandem/config.json` with default providers |
| 6 | Builds the portal with `npm run build` |
| 7 | Writes `examples/agent-quickstart/.env` with token + port |
| 8 | Creates and starts two systemd services: `tandem-engine` + `tandem-agent-portal` |
| 9 | Runs an engine health check and prints the final URL + token |

**Re-running is safe** — existing tokens and provider keys are preserved.

---

## Add an AI provider

After setup, you have two ways to add a provider key:

**Option A — UI (after sign-in):** Go to the **Provider Setup** tab in the portal.

**Option B — edit env file:**
```bash
sudo nano /etc/tandem/engine.env
# Uncomment and fill in one of:
# OPENROUTER_API_KEY=or-...
# OPENAI_API_KEY=sk-...
# ANTHROPIC_API_KEY=sk-ant-...

sudo systemctl restart tandem-engine
```

---

## Environment variables (setup-agent.sh)

| Variable | Default | Description |
|----------|---------|-------------|
| `TANDEM_API_TOKEN` | *(auto-generated)* | Override the generated token |
| `TANDEM_STATE_DIR` | `/srv/tandem` | Engine storage directory |
| `SETUP_ENGINE_AUTO_UPDATE` | `1` | Set to `0` to skip npm update check |

---

## Local dev (no server needed)

```bash
cd examples/agent-quickstart
cp .env.example .env       # set PORTAL_KEY + TANDEM_ENGINE_URL
npm install
npm run dev                # → http://localhost:5173 (Vite dev server with proxy)
```

---

## Portal views

| View | Route | Purpose |
|------|-------|---------|
| **Chat** | `/chat` | Multi-session AI chat with tool results and approval flow |
| **Agents** | `/agents` | Create and manage scheduled routines (cron / interval) |
| **Channels** | `/channels` | Connect Telegram, Discord, or Slack bots |
| **Live Feed** | `/feed` | Global SSE event stream — real-time view of all engine events |

---

## Project structure

```
examples/agent-quickstart/
├── setup-agent.sh        ← one-command setup script
├── server.js             ← Express: Bearer auth + /engine proxy
├── .env.example          ← PORTAL_KEY, TANDEM_ENGINE_URL, PORT
├── package.json
└── src/
    ├── api.ts            # Typed engine API client (~300 lines)
    ├── AuthContext.tsx   # Token persistence + provider status
    ├── runStream.ts      # SSE streaming utility (reconnect / watchdog)
    ├── App.tsx           # Sidebar layout + routing
    └── pages/
        ├── ChatBrain.tsx
        ├── Agents.tsx
        ├── Channels.tsx
        ├── LiveFeed.tsx
        ├── Login.tsx
        └── ProviderSetup.tsx
```
