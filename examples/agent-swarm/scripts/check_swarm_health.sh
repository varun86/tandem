#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${TANDEM_BASE_URL:-http://127.0.0.1:39731}"
API_TOKEN="${TANDEM_API_TOKEN:-}"
TELEGRAM_TOKEN="${TANDEM_TELEGRAM_BOT_TOKEN:-}"
TELEGRAM_CHAT_ID="${TANDEM_SWARM_TELEGRAM_CHAT_ID:-}"
OWNER="${SWARM_GITHUB_OWNER:-}"
REPO="${SWARM_GITHUB_REPO:-}"
STUCK_MINUTES="${SWARM_STUCK_MINUTES:-30}"
RESOURCE_KEY="${SWARM_RESOURCE_KEY:-swarm.active_tasks}"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for check_swarm_health.sh" >&2
  exit 2
fi
if [[ -z "$TELEGRAM_TOKEN" || -z "$TELEGRAM_CHAT_ID" ]]; then
  echo "TANDEM_TELEGRAM_BOT_TOKEN and TANDEM_SWARM_TELEGRAM_CHAT_ID are required" >&2
  exit 2
fi

headers=("-H" "content-type: application/json")
if [[ -n "$API_TOKEN" ]]; then
  headers+=("-H" "authorization: Bearer $API_TOKEN" "-H" "x-tandem-token: $API_TOKEN")
fi

resource_json="$(curl -sS "${headers[@]}" "$BASE_URL/resource/${RESOURCE_KEY}" || true)"
if [[ -z "$resource_json" || "$resource_json" == "null" || "$resource_json" == *"INVALID_RESOURCE_KEY"* ]]; then
  RESOURCE_KEY="project/swarm.active_tasks"
  resource_json="$(curl -sS "${headers[@]}" "$BASE_URL/resource/${RESOURCE_KEY}" || true)"
fi
now_ms="$(date +%s%3N)"
stuck_ms="$((STUCK_MINUTES * 60 * 1000))"

if [[ -z "$resource_json" || "$resource_json" == "null" ]]; then
  summary="Swarm health: no task registry found at ${RESOURCE_KEY}"
else
  tasks_json="$(echo "$resource_json" | jq -c '.value.tasks // {}')"
  total="$(echo "$tasks_json" | jq 'length')"
  blocked_auth="$(echo "$tasks_json" | jq '[to_entries[] | select(.value.blockedBy == "auth")] | length')"
  stuck="$(echo "$tasks_json" | jq --argjson now "$now_ms" --argjson maxIdle "$stuck_ms" '[to_entries[] | select(($now - (.value.lastUpdateMs // 0)) > $maxIdle)] | length')"

  checks_line="checks: skipped"
  if [[ -n "$OWNER" && -n "$REPO" ]]; then
    tool_ids="$(curl -sS "${headers[@]}" "$BASE_URL/tool/ids" || echo '[]')"
    check_tool="$(echo "$tool_ids" | jq -r '.[] | select(test("^mcp\\.arcade\\..*github.*(check|status).*"; "i"))' | head -n1)"
    if [[ -n "$check_tool" ]]; then
      pr_numbers="$(echo "$tasks_json" | jq -r '.[] | .prNumber // empty' | tr '\n' ' ')"
      ok_checks=0
      total_checks=0
      for pr in $pr_numbers; do
        payload="$(jq -nc --arg tool "$check_tool" --arg owner "$OWNER" --arg repo "$REPO" --argjson pr "$pr" '{tool:$tool,args:{owner:$owner,repo:$repo,pull_number:$pr}}')"
        out="$(curl -sS "${headers[@]}" -X POST "$BASE_URL/tool/execute" -d "$payload" || true)"
        total_checks=$((total_checks + 1))
        if echo "$out" | jq -e '.output // .metadata' >/dev/null 2>&1; then
          ok_checks=$((ok_checks + 1))
        fi
      done
      checks_line="checks: $ok_checks/$total_checks queried via $check_tool"
    fi
  fi

  summary="Swarm health (${RESOURCE_KEY})\n- total tasks: $total\n- stuck (>${STUCK_MINUTES}m): $stuck\n- blocked auth: $blocked_auth\n- $checks_line"
fi

curl -sS -X POST "https://api.telegram.org/bot${TELEGRAM_TOKEN}/sendMessage" \
  -H 'content-type: application/json' \
  -d "$(jq -nc --arg chat "$TELEGRAM_CHAT_ID" --arg txt "$summary" '{chat_id:$chat,text:$txt}')" >/dev/null

echo "$summary"
