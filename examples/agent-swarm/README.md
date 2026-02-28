# Agent Swarm (Manager + Workers)

This example runs a Tandem-native swarm with a manager, workers, tester, and reviewer.

It uses:

- Tandem sessions/runs + event bus + shared resources + routines
- One remote MCP server (`arcade`) for GitHub operations
- Telegram for health notifications

## 5-minute quickstart

1. Start Tandem engine.

```bash
cd /home/evan/tandem
npm run engine:start
```

2. Configure env.

```bash
cd examples/agent-swarm
cp .env.example .env
# Fill: TANDEM_SWARM_TELEGRAM_CHAT_ID, TANDEM_TELEGRAM_BOT_TOKEN,
# SWARM_GITHUB_OWNER, SWARM_GITHUB_REPO
# Optional: SWARM_RESOURCE_KEY (default tries swarm.active_tasks then project/swarm.active_tasks)
```

3. Ensure Arcade MCP is connected and GitHub tools are visible.

```bash
curl -sS "$TANDEM_BASE_URL/mcp" | jq
curl -sS "$TANDEM_BASE_URL/tool/ids" | jq '.[] | select(test("^mcp\\.arcade\\..*github"; "i"))'
```

4. Run the manager.

```bash
node ./src/manager.mjs "Implement feature X, add tests, and open PR"
```

5. Create and enable health routine.

```bash
curl -sS -X POST "$TANDEM_BASE_URL/routines" \
  -H "content-type: application/json" \
  -d @./routines/check_swarm_health.json | jq

curl -sS -X POST "$TANDEM_BASE_URL/routines/swarm-health-check/run_now" \
  -H "content-type: application/json" \
  -d '{}' | jq
```

## Demo flow

1. Manager decomposes objective into 1-5 tasks.
2. For each task, manager creates a worktree and worker run, then writes task records to swarm registry (`swarm.active_tasks` or fallback `project/swarm.active_tasks`).
3. Event stream updates task state on run/tool/approval/auth events.
4. Worker completes and task moves to `ready_for_review`.
5. Tester runs lint/tests in same worktree and posts summary.
6. Reviewer checks PR diff/check context and posts approve/reject recommendation.
7. Merge remains manual. Manager never auto-merges.
8. Routine runs every 10 minutes and posts stuck/check summaries to Telegram.

## Registry contract (`swarm.active_tasks`)

If your engine build still enforces prefixed resource namespaces, the example automatically falls back to `project/swarm.active_tasks`.

Each task record includes:

```json
{
  "taskId": "task-1",
  "title": "Implement ...",
  "ownerRole": "worker",
  "status": "running",
  "statusReason": "run started",
  "sessionId": "...",
  "runId": "...",
  "worktreePath": "/repo/.swarm/worktrees/task-1",
  "branch": "swarm/task-1",
  "prUrl": null,
  "prNumber": null,
  "checksStatus": null,
  "lastUpdateMs": 0,
  "blockedBy": null,
  "notifyOnComplete": true
}
```

## Scripts

- `scripts/create_worktree.sh`: creates or reuses a managed worktree and branch.
- `scripts/cleanup_worktrees.sh`: removes only managed worktrees under `.swarm/worktrees`.
- `scripts/check_swarm_health.sh`: finds stuck tasks, queries GitHub checks via MCP, posts to Telegram.

## Routine enablement notes

- Routine file: `routines/check_swarm_health.json`
- Cron: every 10 minutes (`*/10 * * * *`)
- Requires approval: `true`
- External integrations allowed: `true`

## Tests

```bash
npm test
```

Covers:

- deterministic state transitions
- manager task creation + registry update
- idempotent worktree behavior for repeated task IDs
- auth-required blocking without notification loops
