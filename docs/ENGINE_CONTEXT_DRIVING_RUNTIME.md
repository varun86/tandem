# Engine Context-Driving Runtime

## Purpose

This document explains the context-driving runtime that was implemented in Tandem Engine, why it exists, how it differs from orchestration UI surfaces (Command Center/Desktop/TUI), and what reliability benefits it provides for long-running and scheduled runs.

It is the source-of-truth explanation for the context-driving work tracked in [`docs/design/WORKBOARD.md`](./design/WORKBOARD.md) (`CTX-*` and `CTX-ENG-*` items).

## Why This Was Built

Long-running runs and cron/scheduled automations were showing the same failure patterns:

- context drift across many steps
- weak visibility into current run status and next decision
- stale workspace execution risk during directory/workspace switches
- partial failure handling that was hard to recover from deterministically
- UI/client status inconsistencies caused by log-based inference

The core gap was architectural: there was no single, engine-owned state contract that all clients had to follow.

## Core Rule

Engine is the only source of truth.

- Tandem Engine owns canonical `RunState`, step state, blackboard state, checkpoint state, and sequenced events.
- Desktop/TUI are views/controllers only:
  - subscribe to engine state/event APIs
  - render status
  - issue commands
  - do not infer run truth from logs/transcript text

## What Was Built

### 1) Engine-Owned Context Run State Machine

Implemented under engine HTTP routes in `crates/tandem-server/src/http.rs`:

- `POST/GET /context/runs`
- `GET/PUT /context/runs/{run_id}`

Canonical statuses:

- Run status: `queued|planning|running|awaiting_approval|paused|blocked|failed|completed|cancelled`
- Step status: `pending|runnable|in_progress|blocked|done|failed`

### 2) Sequenced Event Stream (Append-Only)

- `POST/GET /context/runs/{run_id}/events`
- `GET /context/runs/{run_id}/events/stream`

Storage:

- `context_runs/<run_id>/events.jsonl`
- monotonic `seq` per run

This enables:

- deterministic event replay
- reconnect-safe streaming (`since_seq`, `tail`)
- auditable decision history

### 3) Workspace Lease Seatbelt

- `POST /context/runs/{run_id}/lease/validate`

Behavior:

- validates current workspace against run lease (`workspace_id`, `canonical_path`, `lease_epoch`)
- on mismatch:
  - emits `workspace_mismatch` event
  - auto-pauses run

This directly prevents stale-directory execution side effects.

### 4) Blackboard (Engine-Owned Working Memory)

- `GET /context/runs/{run_id}/blackboard`
- `POST /context/runs/{run_id}/blackboard/patches`

Storage:

- append-only patches: `blackboard_patches.jsonl`
- materialized view: `blackboard.json`

This provides explicit, inspectable working memory separate from transcript text.

### 5) Checkpointing

- `POST /context/runs/{run_id}/checkpoints`
- `GET /context/runs/{run_id}/checkpoints/latest`

Storage:

- `checkpoints/<seq>.json`

This provides resumable state snapshots for crash/restart and incident workflows.

### 6) Deterministic Replay and Drift Detection

- `GET /context/runs/{run_id}/replay`

Replay behavior:

- replays from latest checkpoint + remaining events (or from events only)
- materializes replayed run state
- compares replayed state vs persisted run state
- returns drift signals (`status_mismatch`, `why_next_step_mismatch`, `step_count_mismatch`)

This is the runtime integrity check: it tells us whether persisted state still matches event history.

### 7) Minimal Meta-Manager (`ContextDriver`)

- `POST /context/runs/{run_id}/driver/next`

Behavior:

- chooses next step from structured step state
- writes required `why_next_step`
- updates step/run status
- emits decision event `meta_next_step_selected`
- supports `dry_run` mode

This is the first engine-level meta-manager loop primitive.

### 8) Todo-to-Step Sync Bridge (`todowrite` Flow)

To connect task planning/execution (`todowrite`) with context-driving state, the engine now supports:

- `POST /context/runs/{run_id}/todos/sync`

What it does:

- maps todo items to canonical context steps (`run.steps`)
- normalizes todo statuses into step statuses (`pending|runnable|in_progress|done|failed|blocked`)
- updates `run.status` and `why_next_step` from synced task state
- emits a structured `todo_synced` context event

Why this matters:

- task updates are no longer only UI/chat artifacts
- plan/task changes become durable engine state transitions
- replay/checkpoint/drift tooling now includes task-list-derived execution context

## Client Integrations

### Desktop

Desktop orchestration surfaces were migrated to engine context-run APIs so UI status comes from engine state/events, not local transcript inference.

### TUI

TUI now supports engine context-run controls/inspection:

- `/context_runs`
- `/context_run_create`
- `/context_run_get`
- `/context_run_events`
- `/context_run_pause|resume|cancel`
- `/context_run_blackboard`
- `/context_run_next`
- `/context_run_replay`
- `/context_run_lineage`
- `/context_run_sync_tasks`
- `/context_run_bind`

`/context_run_lineage` renders `meta_next_step_selected` decisions with `why_next_step` for operator auditability.

`/context_run_bind <run_id>` enables direct flow improvement for `todowrite`:

- bind an active chat agent to a context run
- when `todo.updated` arrives for that agent/session, TUI auto-calls `/context/runs/{run_id}/todos/sync`
- context-run steps stay aligned with the current task list without manual copy steps

## How This Differs from Orchestration / Command Center

Command Center/orchestration and context-driving runtime are different layers.

- Command Center/orchestration:
  - operator UX and control surfaces
  - planning/dispatch interactions
  - visualization

- Context-driving runtime (this work):
  - canonical engine state machine
  - durable event log with sequence guarantees
  - guardrails (workspace lease validation)
  - checkpoints and deterministic replay
  - explicit decision lineage (`why_next_step`)

In short:

- Orchestration UI answers: "What do we show/control?"
- Context-driving runtime answers: "What is true, durable, and recoverable?"

## Reliability Benefits

### Prevents silent drift

- `why_next_step` is explicit and persisted.
- decision events are auditable and queryable.

### Prevents stale workspace side effects

- pre-dispatch/pre-tool-call lease validation can pause run before destructive actions.

### Improves recoverability

- checkpoints + replay make recovery and debugging deterministic.

### Improves cross-client consistency

- Desktop/TUI use same engine state contracts.
- clients do not have separate inferred truths.

### Improves incident forensics

- append-only events + replay/drift checks reveal where state diverged.

## Test and Verification Coverage Added

The implementation added targeted contract/reliability tests in engine and TUI, including:

- context run create/get/events sequencing
- workspace mismatch pause behavior
- replay no-drift and drift detection cases
- ContextDriver:
  - runnable-step selection
  - terminal state handling
  - dry-run non-mutation invariant
  - decision event emission with non-empty `why_next_step`
- todo sync bridge:
  - todo->step mapping and status normalization
  - `todo_synced` event emission
  - fault path compatibility with checkpoint/replay flow
- fault-injection path:
  - decision -> workspace mismatch -> pause -> checkpoint -> replay no drift

## Current Status

Implemented:

- engine-owned context-run reliability substrate
- cross-client (Desktop/TUI) adoption of engine truth
- minimal meta-manager and replay integrity mechanisms
- decision lineage visibility in TUI

Still pending for full operational closure:

- long-duration soak and broader fault-injection campaign with published pass/fail evidence
- richer Desktop/TUI panels for replay/drift and lineage (beyond command-level outputs)

## File-Level Anchors

Primary implementation:

- `crates/tandem-server/src/http.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/sidecar.rs`
- `crates/tandem-tui/src/net/client.rs`
- `crates/tandem-tui/src/app.rs`
- `docs/design/WORKBOARD.md`

## Summary

This work moved Tandem from UI-oriented orchestration coordination to an engine-grounded reliability architecture for long-running execution.

The key shift is not "more UI features." The key shift is canonical, durable, replayable, and guardrailed runtime state that every client consumes consistently.
