---
title: Build an Automation With the AI Assistant
description: Prompt Tandem in a guided conversation to draft, validate, create, and run governed automations.
---

Use this guide when you want the fastest path from human intent to a real `automationsV2` payload.

It is designed for two audiences at once:

- humans in the control panel who want a guided builder
- agents that need to author the same structure from prompts or SDK code

## What this flow proves

The AI-first composer shows that Tandem can do all of the following without changing engine semantics:

- turn a natural-language prompt into a governed plan
- ask a clarification question when the goal is ambiguous
- emit an explicit `automationsV2.create` payload
- preview the resulting JSON or YAML before creation
- create the automation and run it immediately
- keep run history visible in the standard automation views

## The control-panel flow

Open **Automations** and select the **AI Composer** tab if it is enabled for the workspace.

Then follow this rhythm:

1. describe the goal in plain English
2. answer any clarifying question
3. review the generated runbook replay
4. inspect the JSON or YAML payload
5. create the automation
6. run it now if you want to verify the first pass immediately

The composer is intentionally prompt-first. It should feel like talking to a strong operator who can also surface structure.

## What a good prompt looks like

```text
Build a governed automation named "Todo digest + notify" for /workspace/repos/my-repo.
Use a file-reading step to find TODO and FIXME items under src/ and docs/.
Write docs/todo_digest.md with path, line number, and severity.
End with an MCP step that sends a short Slack summary and includes the report path.
Keep the schedule manual for the first pass.
```

If Tandem needs more detail, it should ask a narrow clarification question instead of guessing.

## What Tandem generates

The generated structure should be easy to inspect and reason about:

- `name` and `status`
- `schedule` with a manual, cron, or interval policy
- `workspace_root`
- `agents` with per-agent tool and MCP policy
- `flow.nodes` with explicit dependencies
- `metadata.composer` with provenance for later handoff/debugging

That means you can safely preview, validate, and recreate the same payload from SDK code later.

## Simple example: digest + notify

This is the most direct proof of value: read files, write a report, then notify through MCP.

```ts
const created = await client.automationsV2.create({
  name: "Todo digest + notify",
  status: "active",
  schedule: {
    type: "manual",
    timezone: "UTC",
    misfire_policy: { type: "run_once" },
  },
  workspace_root: "/workspace/repos/my-repo",
  agents: [
    {
      agent_id: "reader",
      display_name: "Reader",
      tool_policy: { allowlist: ["read", "write"] },
      mcp_policy: { allowed_servers: [], allowed_tools: [] },
      approval_policy: "auto",
    },
    {
      agent_id: "notifier",
      display_name: "Notifier",
      tool_policy: { allowlist: ["read"] },
      mcp_policy: { allowed_servers: ["slack"], allowed_tools: ["send_message"] },
      approval_policy: "auto",
    },
  ],
  flow: {
    nodes: [
      {
        node_id: "collect_todos",
        agent_id: "reader",
        objective: "Find TODO and FIXME items under src/ and docs/ with file + line context.",
      },
      {
        node_id: "write_report",
        agent_id: "reader",
        depends_on: ["collect_todos"],
        objective: "Create docs/todo_digest.md with grouped findings and severity ranking.",
      },
      {
        node_id: "notify_team",
        agent_id: "notifier",
        depends_on: ["write_report"],
        objective: "Use MCP to send a short summary to team and include path docs/todo_digest.md.",
      },
    ],
  },
  creator_id: "demo-operator",
});

await client.automationsV2.runNow(created.automation?.automation_id);
```

## Complex example: file scan -> report -> MCP finish

This is the pattern that tends to impress both operators and agent developers:

- read local files first
- produce a durable artifact
- end with an external action through MCP

```python
complex_automation = await client.automations_v2.create({
    "name": "Repo risk radar",
    "status": "active",
    "schedule": {
        "type": "interval",
        "interval_seconds": 12 * 60 * 60,
        "timezone": "UTC",
        "misfire_policy": {"type": "run_once"},
    },
    "workspace_root": "/workspace/repos/my-repo",
    "agents": [
        {
            "agent_id": "scanner",
            "display_name": "Scanner",
            "tool_policy": {"allowlist": ["read"]},
            "mcp_policy": {"allowed_servers": [], "allowed_tools": []},
            "approval_policy": "auto",
        },
        {
            "agent_id": "analyst",
            "display_name": "Analyst",
            "tool_policy": {"allowlist": ["read", "write"]},
            "mcp_policy": {"allowed_servers": [], "allowed_tools": []},
            "approval_policy": "auto",
        },
        {
            "agent_id": "notifier",
            "display_name": "Notifier",
            "tool_policy": {"allowlist": ["read"]},
            "mcp_policy": {"allowed_servers": ["slack"], "allowed_tools": ["send_message"]},
            "approval_policy": "auto",
        },
    ],
    "flow": {
        "nodes": [
            {
                "node_id": "scan_sources",
                "agent_id": "scanner",
                "objective": "Find TODO/FIXME patterns in src/, docs/, and README files.",
            },
            {
                "node_id": "build_risk_report",
                "agent_id": "analyst",
                "depends_on": ["scan_sources"],
                "objective": "Create docs/todo_digest.md with risk tiers, rationale, and exact file references.",
            },
            {
                "node_id": "notify_and_link",
                "agent_id": "notifier",
                "depends_on": ["build_risk_report"],
                "objective": "Send a short Slack summary and include docs/todo_digest.md as the handoff path.",
            },
        ]
    },
})

await client.automations_v2.run_now(complex_automation.automation_id)
```

## Clarification behavior

If the goal is ambiguous, the composer should ask one focused question.

When the question has obvious choices, the UI can render buttons. When it does not, the user can answer in free text.

This is the same behavior agents should emulate when they are generating workflows from prompts:

- prefer a small clarification over a risky assumption
- preserve the chosen branch in the plan conversation
- keep the final payload deterministic once the question is answered

## SDK handoff

If you already know the shape you want, you can skip the conversation and create the payload directly from code.

- [TypeScript SDK](./sdk/typescript/)
- [Python SDK](./sdk/python/)
- [Automation Examples For Teams](./automation-examples-for-teams/)

If you want the conversational starting point plus the code path side by side, use the examples page first and then come back here.

## See also

- [Control Panel (Web Admin)](./control-panel/)
- [Prompting Workflows And Missions](./prompting-workflows-and-missions/)
- [Creating And Running Workflows And Missions](./creating-and-running-workflows-and-missions/)
- [Scheduling Workflows And Automations](./sdk/scheduling-automations/)
