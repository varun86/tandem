# Smart Heartbeat Monitor Pattern

## The Problem With Cron Agents

A common approach for recurring AI work is to run an agent on a cron schedule (e.g., "check my email every 15 minutes" or "monitor the repo for new issues").

This introduces a significant problem: **High Token Spend on Empty Work**. Most of the time, the agent wakes up, reads the state, discovers there is nothing new to do, and terminates. With generative AI models, reading a large amount of state and reasoning "I have done nothing" still costs tokens and execution time.

## The Solution: Triage-First DAG (Smart Heartbeats)

Tandem handles this via the **Smart Heartbeat Monitor Pattern**. Instead of triggering a large, capable workflow to check for work, we separate the automation into a two-node workflow (a DAG) with conditional skipping.

### 1. The Triage Gate (Assess)

The first node in the sequence is a lightweight `assess` node.

- **Model**: Operates on a very cheap model (e.g., Gemini Flash, GPT-4o-mini).
- **Task**: Its only job is to quickly survey the environment (e.g., read an inbox, list new issues, check a channel).
- **Flag**: It is flagged with `metadata.triage_gate: true`.
- **Output**: It outputs a structured JSON `{"has_work": bool}`.

### 2. Transitive Skipping

If the triage gate determines there is no work (`has_work: false`), the Tandem engine intervenes automatically.

- The executor recognizes the output from the triage node.
- It intentionally skips all downstream nodes that depend on that triage node.
- The skipped nodes are marked as complete with an output stating they were skipped (`triage_skipped: true`).
- The automation completes cleanly, quickly, and cheaply.

### 3. Execution Node

If the triage gate determines there _is_ work (`has_work: true`):

- The executor simply proceeds to the next node.
- The downstream nodes leverage larger models to execute complex tasks, acting on the work discovered by the triage node.

## Usage in Tandem

Tandem provides default composition mechanisms and planner teaching sets to understand this pattern natively.

When creating a new workflow via the Control Panel, users can leverage the **Smart Monitoring** pattern by asking for an automation that involves "checking", "monitoring", "watching", or "polling". The planner automatically drafts a 2-stage workflow using `assess` followed by `execute`, setting up the triage gate accurately.

### Planner Prompts and Schema

The compiler recognizes the `assess` step ID, and ensures that it adheres to the strict JSON output expected by the engine.

If you are developing a manually defined `/automations/v2` workflow, set the triage node up with:

```json
{
  "node_id": "assess_work",
  "assigned_agent": "monitor-agent",
  "metadata": {
    "triage_gate": true
  },
  "output_contract": {
    "validator_kind": "structured_json",
    "schema": "{\"type\": \"object\", \"properties\": {\"has_work\": {\"type\": \"boolean\"}}, \"required\": [\"has_work\"]}"
  }
}
```

This ensures full engine-compatibility with the transitive skip features.
