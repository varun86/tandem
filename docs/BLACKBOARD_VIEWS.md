# Blackboard Views Plan (Desktop Orchestrator + Command Center)

## Purpose

Define an implementation-ready UI plan for Blackboard visualization that is consistent with the current engine context-driving runtime.

Primary goals:

- keep AI execution on track with explicit state visibility
- show progress clearly (run + step + task flow)
- show why the AI made each decision (`why_next_step`)
- keep Engine as the single source of truth

## Source-of-Truth Contract

Engine owns truth. Desktop/TUI are views/controllers.

- Truth from engine:
  - `/context/runs/{run_id}` (`status`, `steps`, `why_next_step`)
  - `/context/runs/{run_id}/events` and `/events/stream` (sequenced decisions/activity)
  - `/context/runs/{run_id}/blackboard` (materialized blackboard)
  - `/context/runs/{run_id}/replay` (drift/integrity)
  - `/context/runs/{run_id}/checkpoints/latest` (resume marker)
- UI must not infer status from transcript/log text.

## Scope

In scope:

- Desktop Orchestrator page
- Desktop Command Center page
- shared `BlackboardPanel` component with `docked|expanded|fullscreen` modes

Out of scope:

- chat surfaces
- UI-editing blackboard patches
- alternate non-engine run truth

## Fit With Current Engine System

This plan is aligned to what exists today:

- Sequenced context events with `seq` and SSE stream
- `meta_next_step_selected` events with `why_next_step`
- `todo_synced` events from todo->step sync bridge
- `workspace_mismatch` reliability event
- replay endpoint returning drift flags
- checkpoint latest endpoint

Important implementation note:

- there is no dedicated blackboard patch stream endpoint in current contracts.
- live updates should be driven by `/events/stream`, then refresh blackboard materialized view (`/blackboard`) as needed.

## UX Model

### Mode 1: Docked (default)

Render lightweight overview only (no heavy graph mount):

- run status pill
- current/next step label
- `why_next_step` (1-2 line clamp)
- progress counters:
  - done/total steps
  - blocked count
  - failed count
- recent activity (last 3-5 relevant events)
- alerts:
  - awaiting approval
  - workspace mismatch pause
  - drift detected
- controls:
  - Expand
  - Fullscreen
  - Follow latest toggle
  - clickable alert badges (`drift`, `workspace mismatch`)

### Mode 2: Expanded

Render embedded canvas + inspector:

- graph projection of blackboard/events
- pan/zoom
- basic filters (`kind`, `step_id`, `decision`, `reliability`)
- selected node inspector:
  - title/kind/tags
  - source event seq + ts
  - payload preview
  - linked step + `why_next_step` (if present)
  - event type filter/search context
  - lineage rail option for compact decision list

### Mode 3: Fullscreen

Same as Expanded plus debug controls:

- seq scrubber/jump (latest, previous decision, previous checkpoint)
- checkpoint marker(s)
- replay integrity badge panel (drift flags)
- ESC + visible exit button
  - focus trap inside fullscreen overlay

## Follow Mode Rules

- Follow ON auto-focuses only when a NEW `meta_next_step_selected` decision arrives.
- Non-decision events do not recenter selection.
- Manual navigation (node selection/filter-driven exploration) pauses follow.
- Follow pause is visible and can be re-enabled in one click.
- Selection state is preserved when switching Expanded <-> Fullscreen.

## Blackboard Refresh Policy

- No transcript/log inference; refresh is engine-state/event driven.
- Materialized `/blackboard` refetch only triggers for relevant families:
  - `meta_next_step_selected`
  - `todo_synced`
  - `workspace_mismatch`
  - checkpoint-related events (`*checkpoint*`)
  - run lifecycle transitions (`run_*`) and explicit run status transitions
- Refresh requests are debounced (350ms) to coalesce bursty event streams.
- `last_blackboard_refresh_seq` watermark prevents redundant fetches.
- Both Orchestrator and Command Center use the same refresh policy helpers.

## Drift Drawer Behavior

- Drift badge is actionable; click opens Drift Details drawer.
- Drawer shows:
  - drift flags (`status_mismatch`, `why_next_step_mismatch`, `step_count_mismatch`)
  - checkpoint seq marker
  - last event seq marker
  - copyable debug bundle JSON (`run_id`, replay payload, seq markers, selected node)
- `Esc` closes fullscreen and any open drift drawer.

## Keyboard Shortcuts

- `E`: expand/dock toggle
- `F`: fullscreen toggle
- `Space`: follow toggle
- `/`: focus node search input
- `Esc`: exit fullscreen and close drawers

## Visual Progress and Decision Lineage

To keep execution on track, the panel should expose two explicit tracks:

- Progress track:
  - step state progression (`pending -> runnable -> in_progress -> done/failed/blocked`)
  - task sync milestones (`todo_synced`)
  - run state transitions
- Decision lineage track:
  - ordered `meta_next_step_selected` nodes
  - each node shows:
    - selected step id
    - `why_next_step`
    - resulting run status
    - seq/ts

This makes "what happened" and "why it happened" first-class, not inferred.

## Data Projection Rules (MVP)

### Inputs

- run state: `/context/runs/{run_id}`
- events:
  - initial: `/context/runs/{run_id}/events?tail=N`
  - live: `/context/runs/{run_id}/events/stream?since_seq=...`
- blackboard: `/context/runs/{run_id}/blackboard`
- replay: `/context/runs/{run_id}/replay` (periodic or on-demand)
- checkpoint: `/context/runs/{run_id}/checkpoints/latest`

### Node types

- `decision` from `meta_next_step_selected`
- `memory` from blackboard items (`facts`, `decisions`, `open_questions`, `artifacts`)
- `task_sync` from `todo_synced`
- `reliability` from `workspace_mismatch` and similar reliability events
- `checkpoint` from latest checkpoint metadata

### Parent/edge derivation

- if explicit `viz.parent` metadata exists, honor it
- otherwise:
  - memory/task/reliability nodes attach to most recent decision node by `seq`
  - if none exists, attach to run-root node

## Performance Guardrails

- Never mount heavy canvas in Docked mode
- Use incremental append for live event nodes
- Batch redraws on stream bursts (e.g. 100-250 ms debounce)
- Virtualize long inspector lists
- Preserve camera + selection when switching Expanded <-> Fullscreen
- Docked mode stays lightweight and avoids mounting heavy projected node views

## Integration Plan

### Phase A: Docked parity (both pages)

- Add `BlackboardPanel` in Orchestrator and Command Center
- Docked mode only
- render status/progress/why/alerts from engine contracts

Acceptance:

- both pages show same canonical run truth for same `run_id`
- no transcript-derived state

### Phase B: Expanded graph + lineage

- enable expanded embedded graph
- add decision lineage rail from `meta_next_step_selected`
- add `todo_synced` and reliability event markers

Acceptance:

- operator can explain current run direction from lineage view alone

### Phase C: Fullscreen debug

- add fullscreen + seq scrub + checkpoint/replay indicators
- drift flags visible and actionable

Acceptance:

- operator can debug divergence/recovery path without leaving panel

## Detailed Acceptance Criteria

- Blackboard panel appears in both Orchestrator and Command Center
- Docked mode updates live from event stream without graph mount
- `why_next_step` always visible in docked and inspector contexts
- Progress counters match `RunState.steps`
- Decision lineage orders by `seq`, not wall-clock assumptions
- Drift indicator comes from `/replay` flags only
- Workspace mismatch alert comes from `workspace_mismatch` event
- Fullscreen exits with ESC and preserves view state

## Risks and Mitigations

- Risk: UI drift from engine truth
  - Mitigation: derive all status/lineage/progress from context-run APIs only
- Risk: graph performance degradation on long runs
  - Mitigation: docked lightweight mode + incremental projection + virtualization
- Risk: ambiguous parent links for nodes
  - Mitigation: deterministic fallback to nearest prior decision by `seq`
- Risk: stale blackboard snapshot while events stream
  - Mitigation: refresh materialized blackboard on relevant event families

## Implementation Checklist

- [x] Shared `BlackboardPanel` with 3 modes
- [x] Orchestrator page integration
- [x] Command Center page integration
- [x] Run selection plumbing (`run_id`) parity across both pages
- [x] Docked overview data wiring (run/events/replay/checkpoint)
- [x] Expanded graph projection implementation
- [x] Fullscreen + ESC + state preservation
- [x] UI tests for mode/state transitions
- [x] Contract tests for decision/progress/drift rendering from mock engine payloads

## Implemented Now (Code Map)

- Shared panel + modes:
  - `src/components/orchestrate/BlackboardPanel.tsx`
- Projection + indicator contracts (pure helpers):
- Projection + indicator contracts (pure helpers):
  - `src/components/orchestrate/blackboardProjection.ts`
  - `src/components/orchestrate/blackboardRefreshPolicy.ts`
  - `src/components/orchestrate/blackboardPanelState.ts`
  - `src/components/orchestrate/blackboardUiState.ts`
- Desktop integration points:
  - `src/components/orchestrate/OrchestratorPanel.tsx`
  - `src/components/command-center/CommandCenterPage.tsx`
- Engine-backed replay/checkpoint wiring used by panel:
  - `src-tauri/src/commands.rs`
  - `src-tauri/src/sidecar.rs`
  - `src-tauri/src/lib.rs`
- Contract and state-transition tests:
- Contract and state-transition tests:
  - `src/components/orchestrate/blackboardProjection.test.ts`
  - `src/components/orchestrate/blackboardRefreshPolicy.test.ts`
  - `src/components/orchestrate/blackboardPanelState.test.ts`
  - `src/components/orchestrate/blackboardUiState.test.ts`
  - `tsconfig.tests.json`
  - `package.json` script: `test:blackboard`

## Operator-Facing Result

- Panel shows engine-truth status/progress/lineage and `why_next_step` in both Orchestrator and Command Center.
- Expanded/fullscreen modes expose projected node timeline, filters, and inspector for decision reasoning.
- Drift and checkpoint indicators come from engine replay/checkpoint endpoints (not transcript inference).

## Related Docs

- [`docs/ENGINE_CONTEXT_DRIVING_RUNTIME.md`](./ENGINE_CONTEXT_DRIVING_RUNTIME.md)
- [`docs/design/ENGINE_VS_UI.md`](./design/ENGINE_VS_UI.md)
- [`docs/design/WORKBOARD.md`](./design/WORKBOARD.md)
