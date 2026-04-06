---
title: Connected-Agent Handoffs
description: Configure handoff artifacts, watch conditions, and filesystem scope policies for V2 automations in Tandem.
---

Connected-agent handoffs let a V2 automation declare how it stages artifacts between agents, what filesystem events trigger re-evaluation, and which paths each agent is allowed to touch.

The three fields involved are:

| Field              | Purpose                                                            |
| ------------------ | ------------------------------------------------------------------ |
| `handoff_config`   | Directory layout for inbox / approved / archived handoff artifacts |
| `watch_conditions` | Filesystem conditions that gate or trigger automation behaviour    |
| `scope_policy`     | Filesystem sandbox applied to all agents in the automation         |

All three are optional. Omitting them leaves the automation in its default open-access, no-handoff-staging mode.

## handoff_config

`handoff_config` controls where agents write output artifacts and how those artifacts are promoted through a review flow.

### Shape

```json
{
  "inbox_dir": "shared/handoffs/inbox",
  "approved_dir": "shared/handoffs/approved",
  "archived_dir": "shared/handoffs/archived",
  "auto_approve": true
}
```

All paths are relative to the automation's `workspace_root`.

| Field          | Default                    | Description                                                                                                            |
| -------------- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `inbox_dir`    | `shared/handoffs/inbox`    | Where agents deposit new handoff artifacts                                                                             |
| `approved_dir` | `shared/handoffs/approved` | Where approved artifacts land after review                                                                             |
| `archived_dir` | `shared/handoffs/archived` | Where old artifacts are retired                                                                                        |
| `auto_approve` | `true`                     | When `true`, artifacts move directly to `approved_dir`; when `false`, they wait in `inbox_dir` for a human review step |

### When to use manual approval

Set `auto_approve: false` when:

- the automation produces content that requires human sign-off before downstream agents consume it
- you want an inbox → review → approve loop before archiving

Set `auto_approve: true` (the default) when the automation is fully trusted and downstream consumption should happen immediately.

### HTTP: set handoff_config on create

```bash
curl -sS -X POST http://127.0.0.1:39731/automations/v2 \
  -H "content-type: application/json" \
  -d '{
    "name": "daily-content-pipeline",
    "handoff_config": {
      "inbox_dir": "pipeline/inbox",
      "approved_dir": "pipeline/approved",
      "archived_dir": "pipeline/archived",
      "auto_approve": false
    },
    "schedule": { "type": "interval", "interval_seconds": 86400 }
  }'
```

### HTTP: update handoff_config on an existing automation

```bash
curl -sS -X PATCH http://127.0.0.1:39731/automations/v2/daily-content-pipeline \
  -H "content-type: application/json" \
  -d '{
    "handoff_config": {
      "inbox_dir": "pipeline/inbox",
      "approved_dir": "pipeline/approved",
      "archived_dir": "pipeline/archived",
      "auto_approve": true
    }
  }'
```

## watch_conditions

`watch_conditions` is an array of filesystem rules that the automation evaluator inspects during execution. Each condition describes a path pattern and what the evaluator should look for.

### Shape

```json
{
  "watch_conditions": [
    {
      "path": "shared/handoffs/inbox",
      "condition": "any_file_present"
    },
    {
      "path": "job-search/reports",
      "condition": "modified_since_last_run"
    }
  ]
}
```

Each entry uses prefix-matching against the filesystem under `workspace_root`.

### Watch condition types

| Condition                 | Description                                                         |
| ------------------------- | ------------------------------------------------------------------- |
| `any_file_present`        | Passes when at least one file exists at or under the path           |
| `modified_since_last_run` | Passes when any file was modified since the previous automation run |
| `empty`                   | Passes when no files are present                                    |

### HTTP: set watch_conditions on create

```bash
curl -sS -X POST http://127.0.0.1:39731/automations/v2 \
  -H "content-type: application/json" \
  -d '{
    "name": "inbox-processor",
    "watch_conditions": [
      { "path": "shared/handoffs/inbox", "condition": "any_file_present" }
    ],
    "schedule": { "type": "interval", "interval_seconds": 900 }
  }'
```

## scope_policy

`scope_policy` defines the filesystem sandbox that constrains what agents in the automation may read, write, or access. When omitted, agents have no path restrictions (open policy).

### Shape

```json
{
  "readable_paths": ["shared/", "job-search/reports/"],
  "writable_paths": ["shared/handoffs/inbox/"],
  "denied_paths": [".env", ".tandem/secrets/"],
  "watch_paths": ["shared/handoffs/inbox/"]
}
```

All paths use prefix matching relative to `workspace_root`.

| Field            | Description                                                                |
| ---------------- | -------------------------------------------------------------------------- |
| `readable_paths` | Agents may read files at or under these paths. Empty = all paths readable. |
| `writable_paths` | Agents may write to these paths. Should be a subset of `readable_paths`.   |
| `denied_paths`   | Always blocked, even if also listed in readable/writable. Takes priority.  |
| `watch_paths`    | Paths the watch evaluator may scan. Defaults to `readable_paths` if empty. |

### Priority order

`denied_paths` takes priority over everything else. A path listed in both `readable_paths` and `denied_paths` is always blocked.

### HTTP: apply a scope policy

```bash
curl -sS -X PATCH http://127.0.0.1:39731/automations/v2/daily-content-pipeline \
  -H "content-type: application/json" \
  -d '{
    "scope_policy": {
      "readable_paths": ["shared/", "pipeline/"],
      "writable_paths": ["pipeline/inbox/"],
      "denied_paths": [".env"],
      "watch_paths": ["pipeline/inbox/"]
    }
  }'
```

### Removing a scope policy (open access)

Send `scope_policy: null` to revert to open access:

```bash
curl -sS -X PATCH http://127.0.0.1:39731/automations/v2/daily-content-pipeline \
  -H "content-type: application/json" \
  -d '{ "scope_policy": null }'
```

## Control Panel UI

In the **Edit workflow automation** dialog (Automations page → three-dot menu → Edit), the **Handoffs** tab exposes all three fields:

### Handoff config panel

- **Auto-approve toggle** — switches between immediate promotion (emerald/green) and manual inbox review (amber/yellow)
- **Inbox directory** — editable path field, defaults to `shared/handoffs/inbox`
- **Approved directory** — defaults to `shared/handoffs/approved`
- **Archived directory** — defaults to `shared/handoffs/archived`
- **Reset** button — restores all fields to system defaults

### Scope policy panel

- Shows **Open policy** when no restrictions are set
- When paths are defined, shows coloured badges: denied (red), readable (sky), writable (amber), watch (violet)
- Four multi-line path editors — one path per line, prefix matching
- **Clear** button removes all paths and reverts to open policy

Changes are saved with the rest of the automation when you click **Save**.

## Full example: intake pipeline with manual review

```bash
curl -sS -X POST http://127.0.0.1:39731/automations/v2 \
  -H "content-type: application/json" \
  -d '{
    "name": "daily-intake-pipeline",
    "status": "active",
    "schedule": { "type": "cron", "cron_expression": "0 8 * * *", "timezone": "UTC" },
    "workspace_root": "/workspace/ops",
    "handoff_config": {
      "inbox_dir": "intake/inbox",
      "approved_dir": "intake/approved",
      "archived_dir": "intake/archived",
      "auto_approve": false
    },
    "watch_conditions": [
      { "path": "intake/inbox", "condition": "any_file_present" }
    ],
    "scope_policy": {
      "readable_paths": ["intake/", "shared/context/"],
      "writable_paths": ["intake/inbox/"],
      "denied_paths": [".env", ".tandem/secrets/"],
      "watch_paths": ["intake/inbox/"]
    },
    "agents": [
      {
        "agent_id": "intake",
        "display_name": "Intake",
        "tool_policy": { "allowlist": ["read", "write"], "denylist": [] }
      }
    ],
    "flow": {
      "nodes": [
        {
          "node_id": "ingest",
          "agent_id": "intake",
          "objective": "Read all files in intake/inbox, summarise each, and write a digest to intake/inbox/digest.md."
        }
      ]
    }
  }'
```

Run it immediately:

```bash
curl -sS -X POST http://127.0.0.1:39731/automations/v2/daily-intake-pipeline/run_now \
  -H "content-type: application/json" -d '{}'
```

## WorkflowEditDraft type reference

If you are extending the control panel TypeScript code, the three fields live on `WorkflowEditDraft`:

```ts
interface WorkflowEditDraft {
  // ...other fields...
  handoffConfig: any | null; // maps to handoff_config on the API
  watchConditions: any[]; // maps to watch_conditions on the API
  scopePolicy: any | null; // maps to scope_policy on the API
}
```

`workflowAutomationToEditDraft` reads these from the raw automation object using both camelCase and snake_case variants for forward compatibility.

## See also

- [Creating And Running Workflows And Missions](./creating-and-running-workflows-and-missions/)
- [Control Panel (Web Admin)](./control-panel/)
- [MCP Automated Agents](./mcp-automated-agents/)
- [V2 Automations API](./reference/engine-commands/)
