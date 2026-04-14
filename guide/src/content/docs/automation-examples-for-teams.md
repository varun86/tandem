---
title: Automation Examples for Teams
description: Real examples that prove Tandem can build reliable, governed, repeated workflows from a prompt or via SDK/code.
---

Tandem workflows are designed to be authored in three ways with the same runtime:

- a low-friction prompt in the control-panel automation wizard,
- a prompt-first conversation in the AI Composer tab,
- direct engine SDK calls for deterministic deployment,
- and planned task prompts that generate the same structure automatically.

## Agent quickindex (copy first)

- [TypeScript SDK: automationsV2 examples](../sdk/typescript/)
- [Python SDK: automations_v2 examples](../sdk/python/)
- [Control-panel path: automation wizard + schedule](./control-panel/)
- [AI-first workflow composer](./automation-composer-workflows/)
- [Tandem Wow Demo Playbook](./tandem-wow-demo-playbook/)

The same runtime means the automation behavior is identical, no matter how it was authored.

## 1) The easiest path: AI Composer or automation wizard in the control panel

Use this when you want to show that Tandem can turn a conversational prompt into a governed automation before the user ever touches JSON.

Example prompt:

```text
Build a governed automation named "Todo digest + notify" for /workspace/repos/my-repo.
Use a file-reading step to find TODO and FIXME items under src/ and docs/.
Write docs/todo_digest.md with path, line number, and severity.
End with an MCP step that sends a short Slack summary and includes the report path.
```

What this demonstrates:

- policy-safe multi-agent automation creation
- a final MCP step as the terminating action
- previewable payloads that map cleanly to `automationsV2.create`
- `runNow` plus standard run-history inspection

Use this when you want a fast proof that non-engineers can ship autonomous ops.

1. Open **Automations** and start the **Wizard**.
2. Choose **Workflow** (or the equivalent automated workflow starter).
3. Paste a bounded prompt with an explicit output contract.
4. Review the generated nodes and policies.
5. Run once, then schedule if the first run is successful.

Example prompt that works well as a first wizard pass:

```text
Build an automation named "Todo digest + notify" for workspace "/workspace/repos/my-repo".
Use a two-node flow:
- Node 1 (Reader): find TODO/FIXME entries under src/ and docs/.
  - output: docs/todo_digest.md
  - include file path and line number for each finding
  - rank findings as high/medium/low risk
- Node 2 (Notifier): send a short summary to Slack and include that docs/todo_digest.md was updated
  - only post in #team-ops
  - keep message under 12 lines

Set the schedule to manual for now.
Before recurring runs, show the generated DAG for review.
```

What the engine receives is an authored graph with explicit objectives and tool policies, so run and scale it like any production automation.

## 2) Deterministic path: create, run, and inspect with the TypeScript SDK

Use this when you already know the exact DAG and want repeatability with version control.

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
      skills: [],
      tool_policy: { allowlist: ["read", "write"] },
      mcp_policy: { allowed_servers: [], allowed_tools: [] },
      approval_policy: "auto",
    },
    {
      agent_id: "notifier",
      display_name: "Notifier",
      skills: [],
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

const automationId = created.automation?.automation_id;
await client.automationsV2.runNow(automationId);
const runs = await client.automationsV2.listRuns(automationId, 5);
console.log(runs.runs.map((r) => ({ runId: r.run_id, status: r.status })));
```

Python equivalent:

```python
automation = await client.automations_v2.create({
    "name": "Todo digest + notify",
    "status": "active",
    "schedule": {"type": "manual", "timezone": "UTC", "misfire_policy": {"type": "run_once"}},
    "workspace_root": "/workspace/repos/my-repo",
    "agents": [
        {
            "agent_id": "reader",
            "display_name": "Reader",
            "skills": [],
            "tool_policy": {"allowlist": ["read", "write"]},
            "mcp_policy": {"allowed_servers": [], "allowed_tools": []},
            "approval_policy": "auto",
        },
        {
            "agent_id": "notifier",
            "display_name": "Notifier",
            "skills": [],
            "tool_policy": {"allowlist": ["read"]},
            "mcp_policy": {"allowed_servers": ["slack"], "allowed_tools": ["send_message"]},
            "approval_policy": "auto",
        },
    ],
    "flow": {
        "nodes": [
            {
                "node_id": "collect_todos",
                "agent_id": "reader",
                "objective": "Find TODO and FIXME items under src/ and docs/ with file + line context.",
            },
            {
                "node_id": "write_report",
                "agent_id": "reader",
                "depends_on": ["collect_todos"],
                "objective": "Create docs/todo_digest.md with grouped findings and severity ranking.",
            },
            {
                "node_id": "notify_team",
                "agent_id": "notifier",
                "depends_on": ["write_report"],
                "objective": "Use MCP to send a short summary to team and include path docs/todo_digest.md.",
            },
        ]
    },
    "creator_id": "demo-operator",
})

automation_id = automation.automation_id
await client.automations_v2.run_now(automation_id)
runs = await client.automations_v2.list_runs(automation_id, 5)
print([(r.run_id, r.status) for r in runs.runs])
```

This is the “game changer” pattern because one artifact (the automation graph) is portable and can be reviewed, edited, scheduled, repaired, and replayed.

If you prefer Python, mirror this with `client.automations_v2.create(...)` and `run_now(...)`.

## 3) Complex workflow pattern: file-first analysis then final MCP output

This is the exact shape for “read local context, do structured work, then hand off or notify.”

```ts
const created = await client.automationsV2.create({
  name: "Repo risk radar",
  status: "active",
  schedule: {
    type: "interval",
    interval_seconds: 12 * 60 * 60,
    timezone: "UTC",
    misfire_policy: { type: "run_once" },
  },
  workspace_root: "/workspace/repos/my-repo",
  agents: [
    {
      agent_id: "scanner",
      display_name: "Scanner",
      tool_policy: { allowlist: ["read"] },
      mcp_policy: { allowed_servers: [], allowed_tools: [] },
      approval_policy: "auto",
    },
    {
      agent_id: "analyst",
      display_name: "Analyst",
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
        node_id: "scan_sources",
        agent_id: "scanner",
        objective:
          "Find TODO/FIXME patterns in src/, docs/, and README files. Output the top 25 findings as JSON payload in working notes.",
      },
      {
        node_id: "build_risk_report",
        agent_id: "analyst",
        depends_on: ["scan_sources"],
        objective:
          "Create docs/todo_digest.md with risk tiers, rationale, and exact file references. Include one 'no_action_needed' branch if nothing is critical.",
      },
      {
        node_id: "notify_and_link",
        agent_id: "notifier",
        depends_on: ["build_risk_report"],
        objective:
          "Call slack.send_message with a short run summary and the path docs/todo_digest.md. If report is critical, add a bold alert marker.",
      },
    ],
  },
});

const run = await client.automationsV2.runNow(created.automation?.automation_id);
const status = await client.automationsV2.getRun(run?.run_id);
console.log({
  runId: run?.run_id,
  status: status?.run?.status,
  artifact: "docs/todo_digest.md",
});
```

Python equivalent:

```python
created = await client.automations_v2.create({
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
                "objective": "Find TODO/FIXME patterns in src/, docs/, and README files. Output the top 25 findings as JSON payload in working notes.",
            },
            {
                "node_id": "build_risk_report",
                "agent_id": "analyst",
                "depends_on": ["scan_sources"],
                "objective": "Create docs/todo_digest.md with risk tiers, rationale, and exact file references. Include one 'no_action_needed' branch if nothing is critical.",
            },
            {
                "node_id": "notify_and_link",
                "agent_id": "notifier",
                "depends_on": ["build_risk_report"],
                "objective": "Call slack.send_message with a short run summary and the path docs/todo_digest.md. If report is critical, add a bold alert marker.",
            },
        ]
    },
})

run = await client.automations_v2.run_now(created.automation_id)
status = await client.automations_v2.get_run(run.run_id)
print({
    "run_id": run.run_id,
    "status": status.run.status,
    "artifact": "docs/todo_digest.md",
})
```

This demonstrates all three enterprise requirements in one DAG:

- local file inspection
- deterministic staging artifact creation
- external action through MCP at the end

## 4) Make these examples discoverable to agents

To let your agents pull the docs directly instead of carrying snippets in prompts:

- publish this `guide/src/content/docs` content in your internal docs host, and
- expose that docs source as an MCP-readable source for agents, then point agents at the automation example path.

Then agents can read the same canonical source before compiling new workflows, which avoids stale examples and keeps agent behavior consistent with your team’s standards.

Recommended links for implementation path:

- [Control Panel (Web Admin)](./control-panel/)
- [SDK scheduling guide](./sdk/scheduling-automations/)
- [Prompting Workflows And Missions](./prompting-workflows-and-missions/)
- [Creating And Running Workflows And Missions](./creating-and-running-workflows-and-missions/)
- [MCP Automated Agents](./mcp-automated-agents/)
