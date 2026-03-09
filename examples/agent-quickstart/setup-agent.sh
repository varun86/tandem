#!/usr/bin/env bash
# =============================================================================
#  Tandem Agent Quickstart — Auto-Setup
#  Deprecated: prefer `npm i -g @frumu/tandem-panel && tandem-setup init`
#  Usage: sudo bash setup-agent.sh
#
#  What it does:
#    1. Installs the Tandem Engine (via @frumu/tandem npm package)
#    2. Generates an API token
#    3. Installs and builds this portal
#    4. Creates two systemd services: tandem-engine + tandem-agent-portal
#
#  After setup, open http://<your-ip>/  in a browser and sign in with the
#  token printed at the bottom of this script.
# =============================================================================
set -euo pipefail

# ─── Resolve who will own and run the services ──────────────────────────────
SERVICE_USER="${SUDO_USER:-$USER}"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "$(id -u)" -eq 0 ]]; then
  SUDO_CMD=()
  if [[ -z "${SERVICE_USER:-}" || "$SERVICE_USER" == "root" ]]; then
    SERVICE_USER="${SUDO_USER:-$(logname 2>/dev/null || echo root)}"
  fi
else
  SUDO_CMD=(sudo)
fi

if [[ ! -f "$PROJECT_DIR/package.json" || ! -f "$PROJECT_DIR/server.js" ]]; then
  echo "ERROR: setup-agent.sh must be run from the agent-quickstart directory" >&2
  exit 1
fi

SERVICE_HOME="$(getent passwd "$SERVICE_USER" | cut -d: -f6 || true)"
if [[ -z "$SERVICE_HOME" || ! -d "$SERVICE_HOME" ]]; then
  SERVICE_HOME="/home/$SERVICE_USER"
fi

# ─── Logging helpers ─────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo -e "${GREEN}[setup]${NC} $*"; }
warn() { echo -e "${YELLOW}[setup]${NC} $*"; }
fail() { echo -e "${RED}[setup] ERROR:${NC} $*" >&2; exit 1; }

# ─── PATH that includes nvm, pnpm home, and system bin dirs ──────────────────
SERVICE_PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/snap/bin:$SERVICE_HOME/.local/share/pnpm"
if compgen -G "$SERVICE_HOME/.nvm/versions/node/*/bin" >/dev/null 2>&1; then
  while IFS= read -r bin_dir; do
    SERVICE_PATH="$bin_dir:$SERVICE_PATH"
  done < <(ls -d "$SERVICE_HOME"/.nvm/versions/node/*/bin 2>/dev/null | sort -Vr)
fi

run_as_user() {
  if [[ "$(id -u)" -eq 0 ]]; then
    sudo -u "$SERVICE_USER" env "HOME=$SERVICE_HOME" "PATH=$SERVICE_PATH" "$@"
  else
    env "HOME=$SERVICE_HOME" "PATH=$SERVICE_PATH" "$@"
  fi
}

resolve_cmd() {
  run_as_user bash -c "command -v \"$1\" 2>/dev/null || true"
}

# ─── Resolve node ────────────────────────────────────────────────────────────
resolve_node() {
  local c
  if compgen -G "$SERVICE_HOME/.nvm/versions/node/*/bin/node" >/dev/null 2>&1; then
    while IFS= read -r c; do
      [[ -x "$c" ]] && { echo "$c"; return 0; }
    done < <(ls "$SERVICE_HOME"/.nvm/versions/node/*/bin/node 2>/dev/null | sort -Vr)
  fi
  for c in /usr/local/bin/node /usr/bin/node; do
    [[ -x "$c" ]] && { echo "$c"; return 0; }
  done
  c="$(resolve_cmd node)"; [[ -n "$c" && -x "$c" ]] && { echo "$c"; return 0; }
  return 1
}

# ─── Resolve npm ─────────────────────────────────────────────────────────────
resolve_npm() {
  local c
  if compgen -G "$SERVICE_HOME/.nvm/versions/node/*/bin/npm" >/dev/null 2>&1; then
    while IFS= read -r c; do
      [[ -x "$c" ]] && { echo "$c"; return 0; }
    done < <(ls "$SERVICE_HOME"/.nvm/versions/node/*/bin/npm 2>/dev/null | sort -Vr)
  fi
  for c in /usr/local/bin/npm /usr/bin/npm; do
    [[ -x "$c" ]] && { echo "$c"; return 0; }
  done
  c="$(resolve_cmd npm)"; [[ -n "$c" && -x "$c" ]] && { echo "$c"; return 0; }
  return 1
}

# ─── Resolve npm install prefix ──────────────────────────────────────────────
resolve_npm_prefix() {
  local npm_path="$1" npm_dir prefix
  npm_dir="$(dirname "$npm_path")"
  [[ -x "$npm_dir/node" ]] && { dirname "$npm_dir"; return 0; }
  prefix="$(run_as_user "$npm_path" prefix -g 2>/dev/null || true)"
  [[ -n "$prefix" && "$prefix" != "undefined" ]] && { echo "$prefix"; return 0; }
  return 1
}

# ─── Resolve tandem-engine binary ────────────────────────────────────────────
resolve_tandem_engine() {
  local c
  for c in \
    "${NPM_PREFIX:-}/bin/tandem-engine" \
    "$SERVICE_HOME/.npm-global/bin/tandem-engine" \
    "$SERVICE_HOME/.local/share/pnpm/tandem-engine" \
    "/usr/local/bin/tandem-engine" \
    "/usr/bin/tandem-engine"; do
    [[ -n "$c" && -x "$c" ]] && { echo "$c"; return 0; }
  done
  c="$(resolve_cmd tandem-engine)"; [[ -n "$c" && -x "$c" ]] && { echo "$c"; return 0; }
  return 1
}

# ─── Install @frumu/tandem globally ──────────────────────────────────────────
install_tandem_engine() {
  local npm="$1" prefix="$2"
  log "Installing @frumu/tandem globally via npm…"
  # Clean stale installs first
  run_as_user "$npm" --prefix "$prefix" uninstall -g @frumu/tandem >/dev/null 2>&1 || true
  run_as_user rm -f "$prefix/bin/tandem-engine" >/dev/null 2>&1 || true
  run_as_user "$npm" --prefix "$prefix" install -g @frumu/tandem
}

# =============================================================================
log "Starting Tandem Agent Quickstart setup"
log "Service user: $SERVICE_USER  |  Home: $SERVICE_HOME"
log "Project directory: $PROJECT_DIR"
echo ""

# ─── Require node ────────────────────────────────────────────────────────────
NODE_PATH="$(resolve_node || true)"
[[ -n "$NODE_PATH" ]] || fail "Node.js not found for user '$SERVICE_USER'.
  Install Node.js first:  curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo bash - && sudo apt install -y nodejs"
log "Node:  $NODE_PATH  ($(run_as_user "$NODE_PATH" --version))"

# ─── Require npm ─────────────────────────────────────────────────────────────
NPM_PATH="$(resolve_npm || true)"
[[ -n "$NPM_PATH" ]] || fail "npm not found. Install it: sudo apt install -y npm"
NPM_PREFIX="$(resolve_npm_prefix "$NPM_PATH" || true)"
[[ -n "$NPM_PREFIX" ]] || fail "Could not resolve npm global prefix for $NPM_PATH"
log "npm:   $NPM_PATH  (prefix: $NPM_PREFIX)"

# ─── Install tandem engine if needed ─────────────────────────────────────────
ENGINE_PATH="$(resolve_tandem_engine || true)"
if [[ -z "$ENGINE_PATH" ]]; then
  install_tandem_engine "$NPM_PATH" "$NPM_PREFIX"
  ENGINE_PATH="$(resolve_tandem_engine || true)"
  [[ -n "$ENGINE_PATH" ]] || fail "tandem-engine install failed — check npm output above."
fi

# Upgrade to latest if SETUP_ENGINE_AUTO_UPDATE is not set to 0
if [[ "${SETUP_ENGINE_AUTO_UPDATE:-1}" == "1" ]]; then
  log "Updating @frumu/tandem to latest…"
  run_as_user "$NPM_PATH" --prefix "$NPM_PREFIX" install -g @frumu/tandem@latest >/dev/null 2>&1 || warn "Update check failed; using existing version."
  ENGINE_PATH="$(resolve_tandem_engine || true)"
fi

if [[ -z "$ENGINE_PATH" ]]; then
  ENGINE_PATH="$(resolve_cmd tandem-engine || true)"
fi
[[ -n "$ENGINE_PATH" && -x "$ENGINE_PATH" ]] || fail "tandem-engine binary still missing after install."
log "Engine: $ENGINE_PATH  ($(run_as_user "$ENGINE_PATH" --version 2>/dev/null || echo 'version?'))"

# ─── Generate API token ───────────────────────────────────────────────────────
# Reuse existing .env token if one exists (idempotent installs)
ENGINE_ENV_PATH="/etc/tandem/engine.env"
TOKEN="${TANDEM_API_TOKEN:-}"
if [[ -z "$TOKEN" && -f "$PROJECT_DIR/.env" ]]; then
  TOKEN="$(sed -n 's/^PORTAL_KEY=//p' "$PROJECT_DIR/.env" | tail -n1 || true)"
  [[ -n "$TOKEN" ]] && log "Reusing existing PORTAL_KEY from .env"
fi
if [[ -z "$TOKEN" && -f "$PROJECT_DIR/.env" ]]; then
  TOKEN="$(sed -n 's/^VITE_PORTAL_KEY=//p' "$PROJECT_DIR/.env" | tail -n1 || true)"
  [[ -n "$TOKEN" ]] && log "Reusing existing VITE_PORTAL_KEY from .env"
fi
if [[ -z "$TOKEN" ]]; then
  if "${SUDO_CMD[@]}" test -f "$ENGINE_ENV_PATH"; then
    TOKEN="$("${SUDO_CMD[@]}" sed -n 's/^TANDEM_API_TOKEN=//p' "$ENGINE_ENV_PATH" | tail -n1 || true)"
    [[ -n "$TOKEN" ]] && log "Reusing existing TANDEM_API_TOKEN from $ENGINE_ENV_PATH"
  fi
fi
if [[ -z "$TOKEN" ]]; then
  TOKEN="$(run_as_user "$ENGINE_PATH" token generate 2>/dev/null || true)"
  [[ -n "$TOKEN" ]] || fail "Failed to generate API token. Check engine installation."
  log "Generated new API token."
fi

# ─── Write engine environment file ───────────────────────────────────────────
STATE_DIR="${TANDEM_STATE_DIR:-/srv/tandem}"
ENGINE_CONFIG_PATH="$STATE_DIR/config.json"
TOOL_ROUTER_ENABLED="${TANDEM_TOOL_ROUTER_ENABLED:-0}"
PROMPT_CONTEXT_HOOK_TIMEOUT_MS="${TANDEM_PROMPT_CONTEXT_HOOK_TIMEOUT_MS:-5000}"
PROVIDER_STREAM_CONNECT_TIMEOUT_MS="${TANDEM_PROVIDER_STREAM_CONNECT_TIMEOUT_MS:-90000}"
PROVIDER_STREAM_IDLE_TIMEOUT_MS="${TANDEM_PROVIDER_STREAM_IDLE_TIMEOUT_MS:-90000}"
PERMISSION_WAIT_TIMEOUT_MS="${TANDEM_PERMISSION_WAIT_TIMEOUT_MS:-15000}"
TOOL_EXEC_TIMEOUT_MS="${TANDEM_TOOL_EXEC_TIMEOUT_MS:-45000}"
BASH_TIMEOUT_MS="${TANDEM_BASH_TIMEOUT_MS:-30000}"

"${SUDO_CMD[@]}" mkdir -p /etc/tandem "$STATE_DIR"
"${SUDO_CMD[@]}" chown -R "$SERVICE_USER":"$SERVICE_USER" "$STATE_DIR"

# Preserve custom engine env and provider API keys across re-runs
EXISTING_ENGINE_ENV="$("${SUDO_CMD[@]}" sh -c "test -f '$ENGINE_ENV_PATH' && cat '$ENGINE_ENV_PATH' || true")"
PROVIDER_KEY_REGEX='^(OPENROUTER_API_KEY|OPENAI_API_KEY|ANTHROPIC_API_KEY|GROQ_API_KEY|MISTRAL_API_KEY|COHERE_API_KEY|TOGETHER_API_KEY|GITHUB_TOKEN)='
PRESERVED_ENGINE_ENV="$(printf '%s\n' "$EXISTING_ENGINE_ENV" | grep -Ev '^(TANDEM_API_TOKEN|TANDEM_STATE_DIR|TANDEM_MEMORY_DB_PATH|TANDEM_ENABLE_GLOBAL_MEMORY|TANDEM_TOOL_ROUTER_ENABLED|TANDEM_PROMPT_CONTEXT_HOOK_TIMEOUT_MS|TANDEM_PROVIDER_STREAM_CONNECT_TIMEOUT_MS|TANDEM_PROVIDER_STREAM_IDLE_TIMEOUT_MS|TANDEM_PERMISSION_WAIT_TIMEOUT_MS|TANDEM_TOOL_EXEC_TIMEOUT_MS|TANDEM_BASH_TIMEOUT_MS)=' | grep -Ev "$PROVIDER_KEY_REGEX" || true)"
EXISTING_PROVIDER_ENV="$(printf '%s\n' "$EXISTING_ENGINE_ENV" | grep -E "$PROVIDER_KEY_REGEX" || true)"
# Also pick up keys pre-populated in project .env
PROJECT_PROVIDER_ENV=""
if [[ -f "$PROJECT_DIR/.env" ]]; then
  PROJECT_PROVIDER_ENV="$(grep -E "$PROVIDER_KEY_REGEX" "$PROJECT_DIR/.env" || true)"
fi
# Keep existing provider keys by default, but let project .env override/add values.
MERGED_PROVIDER_ENV="$(
  {
    printf '%s\n' "$EXISTING_PROVIDER_ENV"
    printf '%s\n' "$PROJECT_PROVIDER_ENV"
  } | awk -F= '/^[A-Z0-9_]+=/ {
      key=$1
      if (!(key in seen)) {
        order[++count]=key
        seen[key]=1
      }
      value[key]=$0
    }
    END {
      for (i=1; i<=count; i++) {
        print value[order[i]]
      }
    }'
)"

"${SUDO_CMD[@]}" tee "$ENGINE_ENV_PATH" >/dev/null <<EOF
TANDEM_API_TOKEN=$TOKEN
TANDEM_STATE_DIR=$STATE_DIR
TANDEM_MEMORY_DB_PATH=$STATE_DIR/memory.sqlite
TANDEM_ENABLE_GLOBAL_MEMORY=1
TANDEM_TOOL_ROUTER_ENABLED=$TOOL_ROUTER_ENABLED
TANDEM_PROMPT_CONTEXT_HOOK_TIMEOUT_MS=$PROMPT_CONTEXT_HOOK_TIMEOUT_MS
TANDEM_PROVIDER_STREAM_CONNECT_TIMEOUT_MS=$PROVIDER_STREAM_CONNECT_TIMEOUT_MS
TANDEM_PROVIDER_STREAM_IDLE_TIMEOUT_MS=$PROVIDER_STREAM_IDLE_TIMEOUT_MS
TANDEM_PERMISSION_WAIT_TIMEOUT_MS=$PERMISSION_WAIT_TIMEOUT_MS
TANDEM_TOOL_EXEC_TIMEOUT_MS=$TOOL_EXEC_TIMEOUT_MS
TANDEM_BASH_TIMEOUT_MS=$BASH_TIMEOUT_MS
EOF
if [[ -n "$PRESERVED_ENGINE_ENV" ]]; then
  printf '%s\n' "$PRESERVED_ENGINE_ENV" | "${SUDO_CMD[@]}" tee -a "$ENGINE_ENV_PATH" >/dev/null
fi
if [[ -n "$MERGED_PROVIDER_ENV" ]]; then
  printf '%s\n' "$MERGED_PROVIDER_ENV" | "${SUDO_CMD[@]}" tee -a "$ENGINE_ENV_PATH" >/dev/null
fi
# Add provider key hints if not already present
if ! "${SUDO_CMD[@]}" grep -q '^# OPENROUTER_API_KEY=' "$ENGINE_ENV_PATH"; then
  "${SUDO_CMD[@]}" tee -a "$ENGINE_ENV_PATH" >/dev/null <<'EOF'

# ─── Provider API keys (uncomment at least one) ───────────────────────────
# OPENROUTER_API_KEY=or-...
# OPENAI_API_KEY=sk-...
# ANTHROPIC_API_KEY=sk-ant-...
# GROQ_API_KEY=gsk_...
# MISTRAL_API_KEY=...
EOF
fi

"${SUDO_CMD[@]}" chmod 640 "$ENGINE_ENV_PATH"
"${SUDO_CMD[@]}" chown root:"$SERVICE_USER" "$ENGINE_ENV_PATH"

# Bootstrap engine config (default providers) if missing
if ! "${SUDO_CMD[@]}" test -f "$ENGINE_CONFIG_PATH"; then
  "${SUDO_CMD[@]}" mkdir -p "$(dirname "$ENGINE_CONFIG_PATH")"
  "${SUDO_CMD[@]}" tee "$ENGINE_CONFIG_PATH" >/dev/null <<'EOF'
{
  "default_provider": "openrouter",
  "providers": {
    "openrouter": { "default_model": "google/gemini-2.5-pro-preview" },
    "openai": { "default_model": "gpt-4o-mini" },
    "anthropic": { "default_model": "claude-sonnet-4-5-latest" }
  }
}
EOF
  "${SUDO_CMD[@]}" chown "$SERVICE_USER":"$SERVICE_USER" "$ENGINE_CONFIG_PATH"
  log "Bootstrapped engine config at $ENGINE_CONFIG_PATH"
fi

# ─── Systemd service: tandem-engine ──────────────────────────────────────────
log "Creating systemd service: tandem-engine"
"${SUDO_CMD[@]}" tee /etc/systemd/system/tandem-engine.service >/dev/null <<EOF
[Unit]
Description=Tandem Engine
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_USER
EnvironmentFile=$ENGINE_ENV_PATH
Environment=PATH=$SERVICE_PATH
WorkingDirectory=$STATE_DIR
ExecStart=$ENGINE_PATH serve --hostname 127.0.0.1 --port 39731
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=$STATE_DIR $SERVICE_HOME

[Install]
WantedBy=multi-user.target
EOF

# ─── Build portal ────────────────────────────────────────────────────────────
log "Installing portal npm dependencies…"
cd "$PROJECT_DIR"
# Remove pnpm-linked node_modules if left over from dev
if [[ -d "$PROJECT_DIR/node_modules/.pnpm" || -f "$PROJECT_DIR/node_modules/.modules.yaml" ]]; then
  log "Removing pnpm-style node_modules before npm install"
  run_as_user rm -rf "$PROJECT_DIR/node_modules" "$PROJECT_DIR/package-lock.json"
fi
run_as_user rm -f "$PROJECT_DIR/package-lock.json"   # avoid stale peer conflicts
run_as_user "$NPM_PATH" install
log "Building portal (vite build)…"
run_as_user "$NPM_PATH" run build

# ─── Write portal .env ───────────────────────────────────────────────────────
if [[ ! -f "$PROJECT_DIR/.env" ]]; then
  cat >"$PROJECT_DIR/.env" <<EOF
PORT=80
PORTAL_KEY=$TOKEN
TANDEM_ENGINE_URL=http://127.0.0.1:39731
EOF
fi
# Sync portal key to match engine token (idempotent)
if grep -q '^PORTAL_KEY=' "$PROJECT_DIR/.env"; then
  sed -i "s/^PORTAL_KEY=.*/PORTAL_KEY=$TOKEN/" "$PROJECT_DIR/.env"
else
  echo "PORTAL_KEY=$TOKEN" >>"$PROJECT_DIR/.env"
fi
if grep -q '^VITE_PORTAL_KEY=' "$PROJECT_DIR/.env"; then
  sed -i "s/^VITE_PORTAL_KEY=.*/VITE_PORTAL_KEY=$TOKEN/" "$PROJECT_DIR/.env"
else
  echo "VITE_PORTAL_KEY=$TOKEN" >>"$PROJECT_DIR/.env"
fi
[[ $(grep -c '^PORT=' "$PROJECT_DIR/.env") -eq 0 ]]               && echo "PORT=80" >>"$PROJECT_DIR/.env"
[[ $(grep -c '^TANDEM_ENGINE_URL=' "$PROJECT_DIR/.env") -eq 0 ]]  && echo "TANDEM_ENGINE_URL=http://127.0.0.1:39731" >>"$PROJECT_DIR/.env"

# ─── Systemd service: tandem-agent-portal ────────────────────────────────────
log "Creating systemd service: tandem-agent-portal"
"${SUDO_CMD[@]}" tee /etc/systemd/system/tandem-agent-portal.service >/dev/null <<EOF
[Unit]
Description=Tandem Agent Portal
After=network-online.target tandem-engine.service
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_USER
WorkingDirectory=$PROJECT_DIR
EnvironmentFile=$PROJECT_DIR/.env
ExecStart=$NODE_PATH $PROJECT_DIR/server.js
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

# ─── Enable and start services ───────────────────────────────────────────────
"${SUDO_CMD[@]}" systemctl daemon-reload
"${SUDO_CMD[@]}" systemctl enable --now tandem-engine
"${SUDO_CMD[@]}" systemctl restart tandem-engine
sleep 1

"${SUDO_CMD[@]}" systemctl enable --now tandem-agent-portal
"${SUDO_CMD[@]}" systemctl restart tandem-agent-portal

# ─── Wait for engine health ───────────────────────────────────────────────────
log "Waiting for engine to become healthy…"
HEALTH_OK=0
for i in $(seq 1 12); do
  if run_as_user curl -sf "http://127.0.0.1:39731/global/health" >/dev/null 2>&1; then
    HEALTH_OK=1
    break
  fi
  sleep 2
done
if [[ "$HEALTH_OK" -eq 1 ]]; then
  log "Engine is healthy ✓"
else
  warn "Engine health check timed out — check: sudo journalctl -u tandem-engine -n 30"
fi

# ─── Done! ────────────────────────────────────────────────────────────────────
HAS_PROVIDER_KEY=0
"${SUDO_CMD[@]}" grep -Eq "$PROVIDER_KEY_REGEX" "$ENGINE_ENV_PATH" && HAS_PROVIDER_KEY=1 || true

PUBLIC_IP="$(curl -sf https://api.ipify.org 2>/dev/null || echo '<your-server-ip>')"
PORT_VALUE="$(sed -n 's/^PORT=//p' "$PROJECT_DIR/.env" | tail -n1 | tr -d '[:space:]' || true)"
[[ -n "$PORT_VALUE" ]] || PORT_VALUE="80"

PORTAL_URL="http://$PUBLIC_IP"
if [[ "$PORT_VALUE" == "443" ]]; then
  PORTAL_URL="https://$PUBLIC_IP"
elif [[ "$PORT_VALUE" != "80" ]]; then
  PORTAL_URL="http://$PUBLIC_IP:$PORT_VALUE"
fi

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  ✓ Tandem Agent Quickstart is running!${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "  Portal URL:   ${GREEN}$PORTAL_URL${NC}"
echo -e "  Sign-in key:  ${YELLOW}$TOKEN${NC}"
echo ""
echo "  Services:"
echo "    sudo systemctl status tandem-engine"
echo "    sudo systemctl status tandem-agent-portal"
echo "    sudo journalctl -u tandem-engine -f"
echo ""

if [[ "$HAS_PROVIDER_KEY" -eq 0 ]]; then
  echo -e "${YELLOW}  ⚠  No AI provider key found!${NC}"
  echo "  Edit /etc/tandem/engine.env and set at least one key, e.g.:"
  echo "    OPENROUTER_API_KEY=or-..."
  echo "  Then restart: sudo systemctl restart tandem-engine"
  echo ""
  echo "  Tip: Visit $PUBLIC_IP → Provider Setup to configure a key via the UI once signed in."
fi
